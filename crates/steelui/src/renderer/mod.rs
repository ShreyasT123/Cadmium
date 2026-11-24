use std::collections::HashMap;
use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use cosmic_text::CacheKey;
use wgpu::util::DeviceExt;

use crate::assets::DecodedImage;
use crate::core::{Color, CornerRadii, Rect, SteelError};
use crate::text::{ShapedGlyph, TextBlock, TextEngine, swash_is_color};

pub const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const INITIAL_ATLAS_SIZE: u32 = 256;
const MAX_ATLAS_SIZE: u32 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub usize);

#[derive(Debug, Clone, Copy)]
pub struct SdfQuad {
    pub rect: Rect,
    pub radii: CornerRadii,
    pub fill: Color,
    pub fill_end: Option<Color>,
    pub gradient_dir: [f32; 2],
    pub rotation: f32,
    pub border_color: Color,
    pub border_width: f32,
    pub shadow_color: Color,
    pub shadow_offset: [f32; 2],
    pub shadow_blur: f32,
}

impl Default for SdfQuad {
    fn default() -> Self {
        Self {
            rect: Rect::default(),
            radii: CornerRadii::ZERO,
            fill: Color::TRANSPARENT,
            fill_end: None,
            gradient_dir: [0.0, 1.0],
            rotation: 0.0,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            shadow_color: Color::TRANSPARENT,
            shadow_offset: [0.0, 0.0],
            shadow_blur: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ImageQuad {
    pub rect: Rect,
    pub uv_rect: [f32; 4],
    pub radii: CornerRadii,
    pub tint: Color,
    pub texture: TextureHandle,
}

impl ImageQuad {
    pub fn new(texture: TextureHandle, rect: Rect) -> Self {
        Self {
            rect,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            radii: CornerRadii::ZERO,
            tint: Color::rgba(1.0, 1.0, 1.0, 1.0),
            texture,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlassQuad {
    pub rect: Rect,
    pub radii: CornerRadii,
    pub tint: Color,
    pub border_color: Color,
    pub border_width: f32,
    pub saturation: f32,
    pub specular: f32,
    pub frost: f32,
    pub liquid_ripple: f32,
    pub rim_angle: f32,
    pub sheen_progress: f32,
    pub conic_strength: f32,
}

impl Default for GlassQuad {
    fn default() -> Self {
        Self {
            rect: Rect::default(),
            radii: CornerRadii::ZERO,
            tint: Color::from_rgba8(255, 255, 255, 40),
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            saturation: 1.35,
            specular: 0.90,
            frost: 0.45,
            liquid_ripple: 0.0,
            rim_angle: -1.309,
            sheen_progress: 0.0,
            conic_strength: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalsUniform {
    viewport_size: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniform {
    texel_size: [f32; 2],
    offset: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawSdfInstance {
    rect: [f32; 4],
    radii: [f32; 4],
    fill_color: [f32; 4],
    fill_color_end: [f32; 4],
    border_color: [f32; 4],
    border_and_grad: [f32; 4],
    shadow_color: [f32; 4],
    shadow_params: [f32; 3],
    _pad1: f32,
}

impl From<SdfQuad> for RawSdfInstance {
    fn from(q: SdfQuad) -> Self {
        let end = q.fill_end.unwrap_or(q.fill);
        Self {
            rect: [q.rect.x, q.rect.y, q.rect.width, q.rect.height],
            radii: q.radii.to_array(),
            fill_color: q.fill.to_array(),
            fill_color_end: end.to_array(),
            border_color: q.border_color.to_array(),
            border_and_grad: [
                q.border_width,
                q.gradient_dir[0],
                q.gradient_dir[1],
                q.rotation,
            ],
            shadow_color: q.shadow_color.to_array(),
            shadow_params: [q.shadow_offset[0], q.shadow_offset[1], q.shadow_blur],
            _pad1: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawImageInstance {
    rect: [f32; 4],
    uv_rect: [f32; 4],
    radii: [f32; 4],
    tint: [f32; 4],
}

impl From<ImageQuad> for RawImageInstance {
    fn from(i: ImageQuad) -> Self {
        Self {
            rect: [i.rect.x, i.rect.y, i.rect.width, i.rect.height],
            uv_rect: i.uv_rect,
            radii: i.radii.to_array(),
            tint: i.tint.to_array(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawGlassInstance {
    rect: [f32; 4],
    radii: [f32; 4],
    tint_color: [f32; 4],
    border_color: [f32; 4],
    border_width: f32,
    saturation: f32,
    specular: f32,
    frost: f32,
    glass_fx: [f32; 4],
}

impl From<GlassQuad> for RawGlassInstance {
    fn from(g: GlassQuad) -> Self {
        Self {
            rect: [g.rect.x, g.rect.y, g.rect.width, g.rect.height],
            radii: g.radii.to_array(),
            tint_color: g.tint.to_array(),
            border_color: g.border_color.to_array(),
            border_width: g.border_width,
            saturation: g.saturation,
            specular: g.specular,
            frost: g.frost,
            glass_fx: [
                g.liquid_ripple,
                g.rim_angle,
                g.sheen_progress,
                g.conic_strength,
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawGlyphInstance {
    rect: [f32; 4],
    uv_rect: [f32; 4],
    color: [f32; 4],
}

#[derive(Debug, Clone)]
enum DrawBatch {
    Sdf(Range<u32>),
    Image {
        tex: TextureHandle,
        range: Range<u32>,
    },
    Text(Range<u32>),
    Glass(u32),
    SetScissor(Option<Rect>),
}

pub struct DrawList {
    scale: f32,
    sdf_instances: Vec<RawSdfInstance>,
    image_instances: Vec<RawImageInstance>,
    glyph_instances: Vec<RawGlyphInstance>,
    glass_instances: Vec<RawGlassInstance>,
    clip_stack: Vec<Rect>,
    glyph_scratch: Vec<ShapedGlyph>,
    batches: Vec<DrawBatch>,
    atlas_overflowed: bool,
    has_liquid_glass: bool,
}

impl DrawList {
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
            sdf_instances: Vec::new(),
            image_instances: Vec::new(),
            glyph_instances: Vec::new(),
            glass_instances: Vec::new(),
            clip_stack: Vec::new(),
            glyph_scratch: Vec::new(),
            batches: Vec::new(),
            atlas_overflowed: false,
            has_liquid_glass: false,
        }
    }

    pub fn push_sdf(&mut self, quad: SdfQuad) {
        let idx = self.sdf_instances.len() as u32;
        self.sdf_instances.push(RawSdfInstance::from(quad));
        match self.batches.last_mut() {
            Some(DrawBatch::Sdf(range)) if range.end == idx => range.end = idx + 1,
            _ => self.batches.push(DrawBatch::Sdf(idx..(idx + 1))),
        }
    }

    pub fn push_image(&mut self, img: ImageQuad) {
        let idx = self.image_instances.len() as u32;
        let tex = img.texture;
        self.image_instances.push(RawImageInstance::from(img));
        match self.batches.last_mut() {
            Some(DrawBatch::Image { tex: prev, range }) if *prev == tex && range.end == idx => {
                range.end = idx + 1;
            }
            _ => self.batches.push(DrawBatch::Image {
                tex,
                range: idx..(idx + 1),
            }),
        }
    }

    pub fn push_glass(&mut self, glass: GlassQuad) {
        if glass.liquid_ripple > 0.001 || glass.conic_strength > 0.001 || glass.frost < 0.30 {
            self.has_liquid_glass = true;
        }
        let idx = self.glass_instances.len() as u32;
        self.glass_instances.push(RawGlassInstance::from(glass));
        self.batches.push(DrawBatch::Glass(idx));
    }

    pub fn push_clip(&mut self, rect: Rect) {
        let clipped = match self.clip_stack.last().copied() {
            Some(parent) => intersect_rect(parent, rect),
            None => rect,
        };
        self.clip_stack.push(clipped);
        self.batches.push(DrawBatch::SetScissor(Some(clipped)));
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
        self.batches
            .push(DrawBatch::SetScissor(self.clip_stack.last().copied()));
    }

    pub fn push_text(
        &mut self,
        gpu: &mut GpuContext,
        block: &TextBlock,
        logical_x: f32,
        logical_y: f32,
    ) {
        let origin_x_px = (logical_x * self.scale).round();
        let origin_y_px = (logical_y * self.scale).round();
        block.collect_glyphs_into(
            origin_x_px,
            origin_y_px,
            self.scale,
            &mut self.glyph_scratch,
        );
        let style_color = block.style().color;

        let start = self.glyph_instances.len() as u32;
        for g in &self.glyph_scratch {
            let Some(entry) =
                gpu.glyph_atlas
                    .get_or_insert(&gpu.queue, &mut gpu.text_engine, g.cache_key)
            else {
                if gpu.glyph_atlas.overflow_pending {
                    self.atlas_overflowed = true;
                }
                continue;
            };

            if entry.width == 0 || entry.height == 0 {
                continue;
            }

            let gx = (g.x + entry.left) as f32;
            let gy = (g.y - entry.top) as f32;
            let color = if entry.is_color {
                Color::rgba(1.0, 1.0, 1.0, style_color.a)
            } else {
                style_color
            };

            self.glyph_instances.push(RawGlyphInstance {
                rect: [gx, gy, entry.width as f32, entry.height as f32],
                uv_rect: entry.uv_rect,
                color: color.to_array(),
            });
        }
        let end = self.glyph_instances.len() as u32;

        if end > start {
            match self.batches.last_mut() {
                Some(DrawBatch::Text(range)) if range.end == start => range.end = end,
                _ => self.batches.push(DrawBatch::Text(start..end)),
            }
        }
    }
}

fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width);
    let y1 = (a.y + a.height).min(b.y + b.height);
    Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

struct UploadedTexture {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

#[derive(Clone, Copy)]
struct CachedGlyph {
    uv_rect: [f32; 4],
    width: u32,
    height: u32,
    left: i32,
    top: i32,
    is_color: bool,
}

struct GlyphAtlas {
    size: u32,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    cache: HashMap<CacheKey, Option<CachedGlyph>>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    overflow_pending: bool,
}

impl GlyphAtlas {
    fn new(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler) -> Self {
        let size = INITIAL_ATLAS_SIZE;
        let (texture, bind_group) = Self::create_storage(device, bgl, sampler, size);
        Self {
            size,
            texture,
            bind_group,
            cache: HashMap::new(),
            cursor_x: 1,
            cursor_y: 1,
            row_height: 0,
            overflow_pending: false,
        }
    }

    fn create_storage(
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        size: u32,
    ) -> (wgpu::Texture, wgpu::BindGroup) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SteelUI Glyph Atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ATLAS_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SteelUI Glyph Atlas BG"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        (texture, bind_group)
    }

    fn begin_frame(
        &mut self,
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) {
        if !self.overflow_pending {
            return;
        }
        self.overflow_pending = false;
        if self.size < MAX_ATLAS_SIZE {
            self.size *= 2;
        }
        let (texture, bind_group) = Self::create_storage(device, bgl, sampler, self.size);
        self.texture = texture;
        self.bind_group = bind_group;
        self.cache.clear();
        self.cursor_x = 1;
        self.cursor_y = 1;
        self.row_height = 0;
    }

    fn get_or_insert(
        &mut self,
        queue: &wgpu::Queue,
        text_engine: &mut TextEngine,
        key: CacheKey,
    ) -> Option<CachedGlyph> {
        if let Some(cached) = self.cache.get(&key) {
            return *cached;
        }

        let Some(image) = text_engine.rasterize_uncached(key) else {
            self.cache.insert(key, None);
            return None;
        };

        let w = image.placement.width;
        let h = image.placement.height;
        if w == 0 || h == 0 {
            let empty = CachedGlyph {
                uv_rect: [0.0; 4],
                width: 0,
                height: 0,
                left: image.placement.left,
                top: image.placement.top,
                is_color: false,
            };
            self.cache.insert(key, Some(empty));
            return Some(empty);
        }

        if self.cursor_x + w + 1 > self.size {
            self.cursor_x = 1;
            self.cursor_y += self.row_height + 1;
            self.row_height = 0;
        }
        if self.cursor_y + h + 1 > self.size || w + 2 > self.size {
            self.overflow_pending = true;
            return None;
        }

        let ax = self.cursor_x;
        let ay = self.cursor_y;
        self.cursor_x += w + 1;
        self.row_height = self.row_height.max(h);

        let is_color = swash_is_color(image.content);
        let rgba_bytes: Vec<u8> = match image.content {
            cosmic_text::SwashContent::Mask => {
                let mut buf = Vec::with_capacity((w * h * 4) as usize);
                for &a in &image.data {
                    buf.extend_from_slice(&[255, 255, 255, a]);
                }
                buf
            }
            cosmic_text::SwashContent::SubpixelMask => {
                let mut buf = Vec::with_capacity((w * h * 4) as usize);
                for chunk in image.data.as_chunks::<3>().0 {
                    let a = ((chunk[0] as u16 + chunk[1] as u16 + chunk[2] as u16) / 3) as u8;
                    buf.extend_from_slice(&[255, 255, 255, a]);
                }
                buf
            }
            cosmic_text::SwashContent::Color => image.data,
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: ax, y: ay, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let inv = 1.0 / self.size as f32;
        let entry = CachedGlyph {
            uv_rect: [
                ax as f32 * inv,
                ay as f32 * inv,
                (ax + w) as f32 * inv,
                (ay + h) as f32 * inv,
            ],
            width: w,
            height: h,
            left: image.placement.left,
            top: image.placement.top,
            is_color,
        };
        self.cache.insert(key, Some(entry));
        Some(entry)
    }
}

struct FormatPipelines {
    sdf: wgpu::RenderPipeline,
    text: wgpu::RenderPipeline,
    image: Option<wgpu::RenderPipeline>,
    blur: Option<wgpu::RenderPipeline>,
    glass: Option<wgpu::RenderPipeline>,
    blit: Option<wgpu::RenderPipeline>,
}

pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,

    pub text_engine: TextEngine,
    glyph_atlas: GlyphAtlas,
    textures: Vec<Option<UploadedTexture>>,
    free_textures: Vec<usize>,

    globals_bgl: wgpu::BindGroupLayout,
    tex_bgl: wgpu::BindGroupLayout,
    blur_bgl: wgpu::BindGroupLayout,
    nearest_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,

    sdf_shader: wgpu::ShaderModule,
    text_shader: wgpu::ShaderModule,
    image_shader: Option<wgpu::ShaderModule>,
    blur_shader: Option<wgpu::ShaderModule>,
    glass_shader: Option<wgpu::ShaderModule>,
    blit_shader: Option<wgpu::ShaderModule>,

    pipelines: HashMap<wgpu::TextureFormat, FormatPipelines>,
}

impl GpuContext {
    pub async fn new(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, SteelError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface,
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("SteelUI Device"),
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await?;

        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SteelUI Globals BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let blur_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SteelUI Blur Params BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let (tex_bgl, nearest_sampler, linear_sampler) = create_tex_resources(&device);
        let sdf_shader = device.create_shader_module(wgpu::include_wgsl!("../../shaders/sdf.wgsl"));
        let text_shader =
            device.create_shader_module(wgpu::include_wgsl!("../../shaders/text.wgsl"));
        let glyph_atlas = GlyphAtlas::new(&device, &tex_bgl, &nearest_sampler);

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            text_engine: TextEngine::new(),
            glyph_atlas,
            textures: Vec::new(),
            free_textures: Vec::new(),
            globals_bgl,
            tex_bgl,
            blur_bgl,
            nearest_sampler,
            linear_sampler,
            sdf_shader,
            text_shader,
            image_shader: None,
            blur_shader: None,
            glass_shader: None,
            blit_shader: None,
            pipelines: HashMap::new(),
        })
    }

    pub(crate) fn begin_frame(&mut self) {
        self.glyph_atlas
            .begin_frame(&self.device, &self.tex_bgl, &self.nearest_sampler);
    }

    pub fn upload_image(&mut self, img: &DecodedImage) -> TextureHandle {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SteelUI Image Texture"),
            size: wgpu::Extent3d {
                width: img.width.max(1),
                height: img.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ATLAS_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(img.width * 4),
                rows_per_image: Some(img.height),
            },
            wgpu::Extent3d {
                width: img.width.max(1),
                height: img.height.max(1),
                depth_or_array_layers: 1,
            },
        );

        self.register_texture(texture)
    }

    /// Allocates an offscreen render-target texture for `CustomGpuSurface` and binds it for `ImageQuad`.
    pub fn create_render_texture(
        &mut self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::TextureView, TextureHandle) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SteelUI CustomGpuSurface Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let handle = self.register_texture(texture);
        (view, handle)
    }

    fn register_texture(&mut self, texture: wgpu::Texture) -> TextureHandle {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SteelUI Image BG"),
            layout: &self.tex_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
            ],
        });

        let uploaded = UploadedTexture {
            _texture: texture,
            bind_group,
        };

        if let Some(slot) = self.free_textures.pop() {
            self.textures[slot] = Some(uploaded);
            TextureHandle(slot)
        } else {
            let id = self.textures.len();
            self.textures.push(Some(uploaded));
            TextureHandle(id)
        }
    }

    pub fn release_texture(&mut self, handle: TextureHandle) {
        if let Some(slot) = self.textures.get_mut(handle.0)
            && slot.take().is_some()
        {
            self.free_textures.push(handle.0);
        }
    }

    fn ensure_base_pipelines(&mut self, format: wgpu::TextureFormat) {
        self.pipelines
            .entry(format)
            .or_insert_with(|| FormatPipelines {
                sdf: create_sdf_pipeline(&self.device, &self.sdf_shader, &self.globals_bgl, format),
                text: create_text_pipeline(
                    &self.device,
                    &self.text_shader,
                    &self.globals_bgl,
                    &self.tex_bgl,
                    format,
                ),
                image: None,
                blur: None,
                glass: None,
                blit: None,
            });
    }

    fn ensure_image_pipeline(&mut self, format: wgpu::TextureFormat) {
        self.ensure_base_pipelines(format);
        let shader = self.image_shader.get_or_insert_with(|| {
            self.device
                .create_shader_module(wgpu::include_wgsl!("../../shaders/textured.wgsl"))
        });
        let fp = self.pipelines.get_mut(&format).expect("format pipelines");
        if fp.image.is_none() {
            fp.image = Some(create_image_pipeline(
                &self.device,
                shader,
                &self.globals_bgl,
                &self.tex_bgl,
                format,
            ));
        }
    }

    fn ensure_glass_pipelines(&mut self, format: wgpu::TextureFormat) {
        self.ensure_base_pipelines(format);
        let blur_shader = self.blur_shader.get_or_insert_with(|| {
            self.device
                .create_shader_module(wgpu::include_wgsl!("../../shaders/blur.wgsl"))
        });
        let glass_shader = self.glass_shader.get_or_insert_with(|| {
            self.device
                .create_shader_module(wgpu::include_wgsl!("../../shaders/glass.wgsl"))
        });
        let blit_shader = self.blit_shader.get_or_insert_with(|| {
            self.device
                .create_shader_module(wgpu::include_wgsl!("../../shaders/blit.wgsl"))
        });
        let fp = self.pipelines.get_mut(&format).expect("format pipelines");
        if fp.blur.is_none() {
            fp.blur = Some(create_blur_pipeline(
                &self.device,
                blur_shader,
                &self.tex_bgl,
                &self.blur_bgl,
                format,
            ));
        }
        if fp.glass.is_none() {
            fp.glass = Some(create_glass_pipeline(
                &self.device,
                glass_shader,
                &self.globals_bgl,
                &self.tex_bgl,
                format,
            ));
        }
        if fp.blit.is_none() {
            fp.blit = Some(create_blit_pipeline(
                &self.device,
                blit_shader,
                &self.tex_bgl,
                format,
            ));
        }
    }
}

struct GlassOffscreen {
    format: wgpu::TextureFormat,
    canonical_tex: wgpu::Texture,
    canonical_view: wgpu::TextureView,
    canonical_blit_bg: wgpu::BindGroup,
    canonical_linear_bg: wgpu::BindGroup,

    _mip1_tex: wgpu::Texture,
    mip1_view: wgpu::TextureView,
    mip1_bg: wgpu::BindGroup,

    _mip2_tex: wgpu::Texture,
    mip2_view: wgpu::TextureView,
    mip2_bg: wgpu::BindGroup,

    blur_param_bgs: [wgpu::BindGroup; 3],
    liquid_backdrop: Option<(wgpu::Texture, wgpu::BindGroup)>,
}

impl GlassOffscreen {
    fn new(gpu: &GpuContext, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let (canonical_tex, canonical_view, canonical_blit_bg, canonical_linear_bg) =
            create_canonical_target(gpu, width, height, format);

        let (w1, h1) = ((width / 2).max(1), (height / 2).max(1));
        let (w2, h2) = ((w1 / 2).max(1), (h1 / 2).max(1));

        let (mip1_tex, mip1_view, mip1_bg) = create_blur_target(gpu, w1, h1, format, "BlurMip1");
        let (mip2_tex, mip2_view, mip2_bg) = create_blur_target(gpu, w2, h2, format, "BlurMip2");

        let uniforms = blur_pyramid_uniforms([width, height], [w1, h1], [w2, h2]);
        let blur_param_bgs = std::array::from_fn(|i| {
            let buf = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("SteelUI Blur Params"),
                    contents: bytemuck::bytes_of(&uniforms[i]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SteelUI Blur Params BG"),
                layout: &gpu.blur_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            })
        });

        Self {
            format,
            canonical_tex,
            canonical_view,
            canonical_blit_bg,
            canonical_linear_bg,
            _mip1_tex: mip1_tex,
            mip1_view,
            mip1_bg,
            _mip2_tex: mip2_tex,
            mip2_view,
            mip2_bg,
            blur_param_bgs,
            liquid_backdrop: None,
        }
    }

    fn ensure_liquid_backdrop(&mut self, gpu: &GpuContext, width: u32, height: u32) {
        if self.liquid_backdrop.is_none() {
            self.liquid_backdrop = Some(create_backdrop_target(gpu, width, height, self.format));
        }
    }
}

pub struct WindowRenderTarget {
    width: u32,
    height: u32,
    glass_offscreen: Option<GlassOffscreen>,

    globals_buffer: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,

    sdf_buffer: wgpu::Buffer,
    sdf_capacity: usize,

    image_buffer: wgpu::Buffer,
    image_capacity: usize,

    glyph_buffer: wgpu::Buffer,
    glyph_capacity: usize,

    glass_buffer: wgpu::Buffer,
    glass_capacity: usize,
}

impl WindowRenderTarget {
    pub fn new(gpu: &GpuContext, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);

        let globals = GlobalsUniform {
            viewport_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        let globals_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SteelUI Globals"),
                contents: bytemuck::bytes_of(&globals),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let globals_bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SteelUI Globals BG"),
            layout: &gpu.globals_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        let sdf_capacity = 128;
        let sdf_buffer = create_vertex_buffer::<RawSdfInstance>(&gpu.device, sdf_capacity, "SDF");

        let image_capacity = 32;
        let image_buffer =
            create_vertex_buffer::<RawImageInstance>(&gpu.device, image_capacity, "Image");

        let glyph_capacity = 512;
        let glyph_buffer =
            create_vertex_buffer::<RawGlyphInstance>(&gpu.device, glyph_capacity, "Glyph");

        let glass_capacity = 16;
        let glass_buffer =
            create_vertex_buffer::<RawGlassInstance>(&gpu.device, glass_capacity, "Glass");

        Self {
            width,
            height,
            glass_offscreen: None,
            globals_buffer,
            globals_bg,
            sdf_buffer,
            sdf_capacity,
            image_buffer,
            image_capacity,
            glyph_buffer,
            glyph_capacity,
            glass_buffer,
            glass_capacity,
        }
    }

    pub fn resize(&mut self, gpu: &GpuContext, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        self.glass_offscreen = None;

        let globals = GlobalsUniform {
            viewport_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        gpu.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
    }

    pub fn render(
        &mut self,
        gpu: &mut GpuContext,
        surface_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        clear_color: Color,
        draw_list: &DrawList,
    ) -> bool {
        gpu.ensure_base_pipelines(surface_format);
        if !draw_list.image_instances.is_empty() {
            gpu.ensure_image_pipeline(surface_format);
        }
        let needs_glass = !draw_list.glass_instances.is_empty();
        if needs_glass {
            gpu.ensure_glass_pipelines(surface_format);
        }

        if draw_list.sdf_instances.len() > self.sdf_capacity {
            let new_cap = draw_list.sdf_instances.len().next_power_of_two();
            self.sdf_buffer = create_vertex_buffer::<RawSdfInstance>(&gpu.device, new_cap, "SDF");
            self.sdf_capacity = new_cap;
        }
        if !draw_list.sdf_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.sdf_buffer,
                0,
                bytemuck::cast_slice(&draw_list.sdf_instances),
            );
        }

        if draw_list.image_instances.len() > self.image_capacity {
            let new_cap = draw_list.image_instances.len().next_power_of_two();
            self.image_buffer =
                create_vertex_buffer::<RawImageInstance>(&gpu.device, new_cap, "Image");
            self.image_capacity = new_cap;
        }
        if !draw_list.image_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.image_buffer,
                0,
                bytemuck::cast_slice(&draw_list.image_instances),
            );
        }

        if draw_list.glyph_instances.len() > self.glyph_capacity {
            let new_cap = draw_list.glyph_instances.len().next_power_of_two();
            self.glyph_buffer =
                create_vertex_buffer::<RawGlyphInstance>(&gpu.device, new_cap, "Glyph");
            self.glyph_capacity = new_cap;
        }
        if !draw_list.glyph_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.glyph_buffer,
                0,
                bytemuck::cast_slice(&draw_list.glyph_instances),
            );
        }

        if draw_list.glass_instances.len() > self.glass_capacity {
            let new_cap = draw_list.glass_instances.len().next_power_of_two();
            self.glass_buffer =
                create_vertex_buffer::<RawGlassInstance>(&gpu.device, new_cap, "Glass");
            self.glass_capacity = new_cap;
        }
        if !draw_list.glass_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.glass_buffer,
                0,
                bytemuck::cast_slice(&draw_list.glass_instances),
            );
        }

        if !needs_glass {
            self.glass_offscreen = None;
            return self.render_direct(gpu, surface_view, surface_format, clear_color, draw_list);
        }

        if self
            .glass_offscreen
            .as_ref()
            .is_some_and(|g| g.format != surface_format)
        {
            self.glass_offscreen = None;
        }
        let width = self.width;
        let height = self.height;
        let bufs = BatchBuffers {
            width,
            height,
            globals_bg: &self.globals_bg,
            sdf_buffer: &self.sdf_buffer,
            image_buffer: &self.image_buffer,
            glyph_buffer: &self.glyph_buffer,
        };
        let go = self
            .glass_offscreen
            .get_or_insert_with(|| GlassOffscreen::new(gpu, width, height, surface_format));
        if draw_list.has_liquid_glass {
            go.ensure_liquid_backdrop(gpu, width, height);
        } else {
            go.liquid_backdrop = None;
        }

        let pipes = &gpu.pipelines[&surface_format];
        let blur_pipe = pipes.blur.as_ref().expect("blur pipeline");
        let glass_pipe = pipes.glass.as_ref().expect("glass pipeline");
        let blit_pipe = pipes.blit.as_ref().expect("blit pipeline");

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SteelUI Glass Frame Encoder"),
            });

        let mut is_first_pass = true;
        let mut active_scissor: Option<Rect> = None;
        let mut i = 0;

        while i < draw_list.batches.len() {
            match &draw_list.batches[i] {
                DrawBatch::SetScissor(sc) => {
                    active_scissor = *sc;
                    i += 1;
                }
                DrawBatch::Glass(idx) => {
                    if is_first_pass {
                        let _ = begin_ui_pass(
                            &mut encoder,
                            &go.canonical_view,
                            wgpu::LoadOp::Clear(clear_color.to_wgpu()),
                        );
                        is_first_pass = false;
                    }

                    if let Some((backdrop_tex, _)) = &go.liquid_backdrop {
                        encoder.copy_texture_to_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: &go.canonical_tex,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            wgpu::TexelCopyTextureInfo {
                                texture: backdrop_tex,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            wgpu::Extent3d {
                                width,
                                height,
                                depth_or_array_layers: 1,
                            },
                        );
                    }

                    run_blur_pass(
                        &mut encoder,
                        blur_pipe,
                        &go.canonical_linear_bg,
                        &go.blur_param_bgs[0],
                        &go.mip1_view,
                    );
                    run_blur_pass(
                        &mut encoder,
                        blur_pipe,
                        &go.mip1_bg,
                        &go.blur_param_bgs[1],
                        &go.mip2_view,
                    );
                    run_blur_pass(
                        &mut encoder,
                        blur_pipe,
                        &go.mip2_bg,
                        &go.blur_param_bgs[2],
                        &go.mip1_view,
                    );

                    {
                        let mut pass =
                            begin_ui_pass(&mut encoder, &go.canonical_view, wgpu::LoadOp::Load);
                        if apply_scissor(&mut pass, active_scissor, draw_list.scale, width, height)
                        {
                            let sharp_bg = go
                                .liquid_backdrop
                                .as_ref()
                                .map(|(_, bg)| bg)
                                .unwrap_or(&go.mip1_bg);
                            pass.set_pipeline(glass_pipe);
                            pass.set_bind_group(0, &self.globals_bg, &[]);
                            pass.set_bind_group(1, sharp_bg, &[]);
                            pass.set_bind_group(2, &go.mip1_bg, &[]);
                            pass.set_vertex_buffer(0, self.glass_buffer.slice(..));
                            pass.draw(0..4, *idx..(*idx + 1));
                        }
                    }
                    i += 1;
                }
                DrawBatch::Sdf(_) | DrawBatch::Image { .. } | DrawBatch::Text(_) => {
                    let load = if is_first_pass {
                        is_first_pass = false;
                        wgpu::LoadOp::Clear(clear_color.to_wgpu())
                    } else {
                        wgpu::LoadOp::Load
                    };

                    {
                        let mut pass = begin_ui_pass(&mut encoder, &go.canonical_view, load);
                        i = record_standard_batches(
                            &mut pass,
                            pipes,
                            gpu,
                            &bufs,
                            draw_list,
                            i,
                            &mut active_scissor,
                        );
                    }
                }
            }
        }

        if is_first_pass {
            let _ = begin_ui_pass(
                &mut encoder,
                &go.canonical_view,
                wgpu::LoadOp::Clear(clear_color.to_wgpu()),
            );
        }

        {
            let mut blit_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SteelUI Final Blit Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            blit_pass.set_pipeline(blit_pipe);
            blit_pass.set_bind_group(0, &go.canonical_blit_bg, &[]);
            blit_pass.draw(0..3, 0..1);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        draw_list.atlas_overflowed
    }

    fn render_direct(
        &self,
        gpu: &GpuContext,
        surface_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        clear_color: Color,
        draw_list: &DrawList,
    ) -> bool {
        let pipes = &gpu.pipelines[&surface_format];
        let bufs = BatchBuffers {
            width: self.width,
            height: self.height,
            globals_bg: &self.globals_bg,
            sdf_buffer: &self.sdf_buffer,
            image_buffer: &self.image_buffer,
            glyph_buffer: &self.glyph_buffer,
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SteelUI Direct Frame Encoder"),
            });

        {
            let mut pass = begin_ui_pass(
                &mut encoder,
                surface_view,
                wgpu::LoadOp::Clear(clear_color.to_wgpu()),
            );
            let mut active_scissor: Option<Rect> = None;
            let _ = record_standard_batches(
                &mut pass,
                pipes,
                gpu,
                &bufs,
                draw_list,
                0,
                &mut active_scissor,
            );
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        draw_list.atlas_overflowed
    }
}

struct BatchBuffers<'a> {
    width: u32,
    height: u32,
    globals_bg: &'a wgpu::BindGroup,
    sdf_buffer: &'a wgpu::Buffer,
    image_buffer: &'a wgpu::Buffer,
    glyph_buffer: &'a wgpu::Buffer,
}

fn record_standard_batches<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    pipes: &'a FormatPipelines,
    gpu: &'a GpuContext,
    bufs: &BatchBuffers<'a>,
    draw_list: &DrawList,
    mut i: usize,
    active_scissor: &mut Option<Rect>,
) -> usize {
    let mut scissor_visible = apply_scissor(
        pass,
        *active_scissor,
        draw_list.scale,
        bufs.width,
        bufs.height,
    );
    while i < draw_list.batches.len() {
        match &draw_list.batches[i] {
            DrawBatch::SetScissor(sc) => {
                *active_scissor = *sc;
                scissor_visible = apply_scissor(
                    pass,
                    *active_scissor,
                    draw_list.scale,
                    bufs.width,
                    bufs.height,
                );
                i += 1;
            }
            DrawBatch::Sdf(range) => {
                if scissor_visible {
                    pass.set_pipeline(&pipes.sdf);
                    pass.set_bind_group(0, bufs.globals_bg, &[]);
                    pass.set_vertex_buffer(0, bufs.sdf_buffer.slice(..));
                    pass.draw(0..4, range.clone());
                }
                i += 1;
            }
            DrawBatch::Image { tex, range } => {
                if scissor_visible
                    && let Some(img_pipe) = &pipes.image
                    && let Some(Some(uploaded)) = gpu.textures.get(tex.0)
                {
                    pass.set_pipeline(img_pipe);
                    pass.set_bind_group(0, bufs.globals_bg, &[]);
                    pass.set_bind_group(1, &uploaded.bind_group, &[]);
                    pass.set_vertex_buffer(0, bufs.image_buffer.slice(..));
                    pass.draw(0..4, range.clone());
                }
                i += 1;
            }
            DrawBatch::Text(range) => {
                if scissor_visible {
                    pass.set_pipeline(&pipes.text);
                    pass.set_bind_group(0, bufs.globals_bg, &[]);
                    pass.set_bind_group(1, &gpu.glyph_atlas.bind_group, &[]);
                    pass.set_vertex_buffer(0, bufs.glyph_buffer.slice(..));
                    pass.draw(0..4, range.clone());
                }
                i += 1;
            }
            DrawBatch::Glass(_) => break,
        }
    }
    i
}

fn apply_scissor(
    pass: &mut wgpu::RenderPass<'_>,
    scissor: Option<Rect>,
    scale: f32,
    target_width: u32,
    target_height: u32,
) -> bool {
    match scissor {
        Some(r) => {
            let tw = target_width as f32;
            let th = target_height as f32;
            let x0 = (r.x * scale).round().clamp(0.0, tw) as u32;
            let y0 = (r.y * scale).round().clamp(0.0, th) as u32;
            let x1 = ((r.x + r.width) * scale).round().clamp(0.0, tw) as u32;
            let y1 = ((r.y + r.height) * scale).round().clamp(0.0, th) as u32;
            if x1 <= x0 || y1 <= y0 {
                return false;
            }
            pass.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
            true
        }
        None => {
            pass.set_scissor_rect(0, 0, target_width.max(1), target_height.max(1));
            true
        }
    }
}

fn blur_pyramid_uniforms(full: [u32; 2], mip1: [u32; 2], mip2: [u32; 2]) -> [BlurUniform; 3] {
    [
        BlurUniform {
            texel_size: [1.0 / full[0] as f32, 1.0 / full[1] as f32],
            offset: 0.60,
            _pad: 0.0,
        },
        BlurUniform {
            texel_size: [1.0 / mip1[0] as f32, 1.0 / mip1[1] as f32],
            offset: 0.50,
            _pad: 0.0,
        },
        BlurUniform {
            texel_size: [1.0 / mip2[0] as f32, 1.0 / mip2[1] as f32],
            offset: 0.35,
            _pad: 0.0,
        },
    ]
}

fn begin_ui_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SteelUI UI Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    })
}

fn run_blur_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    src_bg: &wgpu::BindGroup,
    params_bg: &wgpu::BindGroup,
    dst_view: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SteelUI Kawase Blur Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: dst_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, src_bg, &[]);
    pass.set_bind_group(1, params_bg, &[]);
    pass.draw(0..3, 0..1);
}

fn create_vertex_buffer<T>(device: &wgpu::Device, capacity: usize, tag: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(tag),
        size: (capacity * std::mem::size_of::<T>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_canonical_target(
    gpu: &GpuContext,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::BindGroup,
    wgpu::BindGroup,
) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SteelUI Canonical Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let blit_bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("SteelUI Blit BG"),
        layout: &gpu.tex_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&gpu.nearest_sampler),
            },
        ],
    });

    let linear_bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("SteelUI Canonical Linear BG"),
        layout: &gpu.tex_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&gpu.linear_sampler),
            },
        ],
    });

    (texture, view, blit_bg, linear_bg)
}

fn create_blur_target(
    gpu: &GpuContext,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: &gpu.tex_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&gpu.linear_sampler),
            },
        ],
    });

    (texture, view, bg)
}

fn create_backdrop_target(
    gpu: &GpuContext,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SteelUI Sharp Backdrop Copy"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("SteelUI Sharp Backdrop BG"),
        layout: &gpu.tex_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&gpu.linear_sampler),
            },
        ],
    });

    (texture, bg)
}

fn create_sdf_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    globals_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI SDF Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawSdfInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 64,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 80,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 96,
                shader_location: 6,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 112,
                shader_location: 7,
                format: wgpu::VertexFormat::Float32x3,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI SDF Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_image_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    globals_bgl: &wgpu::BindGroupLayout,
    tex_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Image Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl), Some(tex_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawImageInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Image Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_text_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    globals_bgl: &wgpu::BindGroupLayout,
    atlas_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Text Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl), Some(atlas_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawGlyphInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Text Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_blur_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    tex_bgl: &wgpu::BindGroupLayout,
    blur_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Blur Pipeline Layout"),
        bind_group_layouts: &[Some(tex_bgl), Some(blur_bgl)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Blur Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_glass_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    globals_bgl: &wgpu::BindGroupLayout,
    tex_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Glass Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl), Some(tex_bgl), Some(tex_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawGlassInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 64,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 68,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 72,
                shader_location: 6,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 76,
                shader_location: 7,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 80,
                shader_location: 8,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Glass Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_blit_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    tex_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Blit Pipeline Layout"),
        bind_group_layouts: &[Some(tex_bgl)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Blit Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_tex_resources(
    device: &wgpu::Device,
) -> (wgpu::BindGroupLayout, wgpu::Sampler, wgpu::Sampler) {
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("SteelUI Texture BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("SteelUI Nearest Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let linear = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("SteelUI Linear Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });

    (bgl, nearest, linear)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_stack_intersects_and_restores_parent_clip() {
        let mut dl = DrawList::new(1.0);
        dl.push_clip(Rect::new(10.0, 10.0, 200.0, 200.0));
        dl.push_clip(Rect::new(150.0, 50.0, 100.0, 100.0));
        assert_eq!(
            dl.clip_stack.last().copied(),
            Some(Rect::new(150.0, 50.0, 60.0, 100.0))
        );
        dl.pop_clip();
        assert_eq!(
            dl.clip_stack.last().copied(),
            Some(Rect::new(10.0, 10.0, 200.0, 200.0))
        );
        dl.pop_clip();
        assert_eq!(dl.clip_stack.last().copied(), None);
    }
}
