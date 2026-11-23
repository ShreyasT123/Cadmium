use std::collections::HashMap;
use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use cosmic_text::CacheKey;
use wgpu::util::DeviceExt;

use crate::assets::DecodedImage;
use crate::core::{Color, CornerRadii, Rect, SteelError};
use crate::text::{swash_is_color, TextBlock, TextEngine};

pub const CANONICAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const ATLAS_SIZE: u32 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub usize);

#[derive(Debug, Clone, Copy)]
pub struct SdfQuad {
    pub rect: Rect,
    pub radii: CornerRadii,
    pub fill: Color,
    pub fill_end: Option<Color>,
    pub gradient_dir: [f32; 2],
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
            rim_angle: -1.309, // -75 deg
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
    border_width: f32,
    gradient_dir: [f32; 2],
    _pad0: f32,
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
            border_width: q.border_width,
            gradient_dir: q.gradient_dir,
            _pad0: 0.0,
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
            glass_fx: [g.liquid_ripple, g.rim_angle, g.sheen_progress, g.conic_strength],
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
    Image { tex: TextureHandle, range: Range<u32> },
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
    batches: Vec<DrawBatch>,
}

impl DrawList {
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
            sdf_instances: Vec::new(),
            image_instances: Vec::new(),
            glyph_instances: Vec::new(),
            glass_instances: Vec::new(),
            batches: Vec::new(),
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
        let idx = self.glass_instances.len() as u32;
        self.glass_instances.push(RawGlassInstance::from(glass));
        self.batches.push(DrawBatch::Glass(idx));
    }

    pub fn push_clip(&mut self, rect: Rect) {
        self.batches.push(DrawBatch::SetScissor(Some(rect)));
    }

    pub fn pop_clip(&mut self) {
        self.batches.push(DrawBatch::SetScissor(None));
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
        let glyphs = block.collect_glyphs(origin_x_px, origin_y_px, self.scale);
        let style_color = block.style().color;

        let start = self.glyph_instances.len() as u32;
        for g in glyphs {
            let Some(entry) = gpu.glyph_atlas.get_or_insert(
                &gpu.queue,
                &mut gpu.text_engine,
                g.cache_key,
            ) else {
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
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    cache: HashMap<CacheKey, Option<CachedGlyph>>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl GlyphAtlas {
    fn new(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SteelUI Glyph Atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CANONICAL_FORMAT,
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

        Self {
            texture,
            bind_group,
            cache: HashMap::new(),
            cursor_x: 1,
            cursor_y: 1,
            row_height: 0,
        }
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

        if self.cursor_x + w + 1 > ATLAS_SIZE {
            self.cursor_x = 1;
            self.cursor_y += self.row_height + 1;
            self.row_height = 0;
        }
        if self.cursor_y + h + 1 > ATLAS_SIZE {
            self.cache.clear();
            self.cursor_x = 1;
            self.cursor_y = 1;
            self.row_height = 0;
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
                for chunk in image.data.chunks_exact(3) {
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

        let inv = 1.0 / ATLAS_SIZE as f32;
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

pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,

    pub text_engine: TextEngine,
    glyph_atlas: GlyphAtlas,
    textures: Vec<UploadedTexture>,

    globals_bgl: wgpu::BindGroupLayout,
    tex_bgl: wgpu::BindGroupLayout,
    blur_bgl: wgpu::BindGroupLayout,
    nearest_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,

    sdf_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    glass_pipeline: wgpu::RenderPipeline,

    blit_shader: wgpu::ShaderModule,
    blit_pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

impl GpuContext {
    pub async fn new(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, SteelError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
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
                memory_hints: Default::default(),
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

        let (blit_shader, tex_bgl, nearest_sampler, linear_sampler) = create_tex_resources(&device);
        let sdf_pipeline = create_sdf_pipeline(&device, &globals_bgl);
        let image_pipeline = create_image_pipeline(&device, &globals_bgl, &tex_bgl);
        let text_pipeline = create_text_pipeline(&device, &globals_bgl, &tex_bgl);
        let blur_pipeline = create_blur_pipeline(&device, &tex_bgl, &blur_bgl);
        let glass_pipeline = create_glass_pipeline(&device, &globals_bgl, &tex_bgl);
        let glyph_atlas = GlyphAtlas::new(&device, &tex_bgl, &nearest_sampler);

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            text_engine: TextEngine::new(),
            glyph_atlas,
            textures: Vec::new(),
            globals_bgl,
            tex_bgl,
            blur_bgl,
            nearest_sampler,
            linear_sampler,
            sdf_pipeline,
            image_pipeline,
            text_pipeline,
            blur_pipeline,
            glass_pipeline,
            blit_shader,
            blit_pipelines: HashMap::new(),
        })
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
            format: CANONICAL_FORMAT,
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

        let id = TextureHandle(self.textures.len());
        self.textures.push(UploadedTexture {
            _texture: texture,
            bind_group,
        });
        id
    }

    fn blit_pipeline_for(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.blit_pipelines.entry(format).or_insert_with(|| {
            let layout = self
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("SteelUI Blit Pipeline Layout"),
                    bind_group_layouts: &[Some(&self.tex_bgl)],
                    immediate_size: 0,
                });

            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("SteelUI Blit Pipeline"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &self.blit_shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &self.blit_shader,
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
        })
    }
}

pub struct WindowRenderTarget {
    width: u32,
    height: u32,
    canonical_tex: wgpu::Texture,
    canonical_view: wgpu::TextureView,
    canonical_blit_bg: wgpu::BindGroup,
    canonical_linear_bg: wgpu::BindGroup,

    backdrop_tex: wgpu::Texture,
    backdrop_bg: wgpu::BindGroup,

    _mip1_tex: wgpu::Texture,
    mip1_view: wgpu::TextureView,
    mip1_bg: wgpu::BindGroup,

    _mip2_tex: wgpu::Texture,
    mip2_view: wgpu::TextureView,
    mip2_bg: wgpu::BindGroup,

    _mip3_tex: wgpu::Texture,
    mip3_view: wgpu::TextureView,
    mip3_bg: wgpu::BindGroup,

    blur_param_buffers: [wgpu::Buffer; 5],
    blur_param_bgs: [wgpu::BindGroup; 5],

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

        let (canonical_tex, canonical_view, canonical_blit_bg, canonical_linear_bg) =
            create_canonical_target(gpu, width, height);
        let (backdrop_tex, backdrop_bg) = create_backdrop_target(gpu, width, height);

        let (w1, h1) = ((width / 2).max(1), (height / 2).max(1));
        let (w2, h2) = ((w1 / 2).max(1), (h1 / 2).max(1));
        let (w3, h3) = ((w2 / 2).max(1), (h2 / 2).max(1));

        let (mip1_tex, mip1_view, mip1_bg) = create_blur_target(gpu, w1, h1, "BlurMip1");
        let (mip2_tex, mip2_view, mip2_bg) = create_blur_target(gpu, w2, h2, "BlurMip2");
        let (mip3_tex, mip3_view, mip3_bg) = create_blur_target(gpu, w3, h3, "BlurMip3");

        let uniforms = blur_pyramid_uniforms(width, height, w1, h1, w2, h2, w3, h3);
        let blur_param_buffers = uniforms.map(|u| {
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("SteelUI Blur Params"),
                    contents: bytemuck::bytes_of(&u),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                })
        });
        let blur_param_bgs = std::array::from_fn(|i| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SteelUI Blur Params BG"),
                layout: &gpu.blur_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: blur_param_buffers[i].as_entire_binding(),
                }],
            })
        });

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

        let sdf_capacity = 256;
        let sdf_buffer = create_vertex_buffer::<RawSdfInstance>(&gpu.device, sdf_capacity, "SDF");

        let image_capacity = 64;
        let image_buffer =
            create_vertex_buffer::<RawImageInstance>(&gpu.device, image_capacity, "Image");

        let glyph_capacity = 1024;
        let glyph_buffer =
            create_vertex_buffer::<RawGlyphInstance>(&gpu.device, glyph_capacity, "Glyph");

        let glass_capacity = 32;
        let glass_buffer =
            create_vertex_buffer::<RawGlassInstance>(&gpu.device, glass_capacity, "Glass");

        Self {
            width,
            height,
            canonical_tex,
            canonical_view,
            canonical_blit_bg,
            canonical_linear_bg,
            backdrop_tex,
            backdrop_bg,
            _mip1_tex: mip1_tex,
            mip1_view,
            mip1_bg,
            _mip2_tex: mip2_tex,
            mip2_view,
            mip2_bg,
            _mip3_tex: mip3_tex,
            mip3_view,
            mip3_bg,
            blur_param_buffers,
            blur_param_bgs,
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

        let (tex, view, blit_bg, linear_bg) = create_canonical_target(gpu, width, height);
        let (btex, bbg) = create_backdrop_target(gpu, width, height);
        self.canonical_tex = tex;
        self.canonical_view = view;
        self.canonical_blit_bg = blit_bg;
        self.canonical_linear_bg = linear_bg;
        self.backdrop_tex = btex;
        self.backdrop_bg = bbg;

        let (w1, h1) = ((width / 2).max(1), (height / 2).max(1));
        let (w2, h2) = ((w1 / 2).max(1), (h1 / 2).max(1));
        let (w3, h3) = ((w2 / 2).max(1), (h2 / 2).max(1));

        let (m1, mv1, mb1) = create_blur_target(gpu, w1, h1, "BlurMip1");
        let (m2, mv2, mb2) = create_blur_target(gpu, w2, h2, "BlurMip2");
        let (m3, mv3, mb3) = create_blur_target(gpu, w3, h3, "BlurMip3");
        self._mip1_tex = m1;
        self.mip1_view = mv1;
        self.mip1_bg = mb1;
        self._mip2_tex = m2;
        self.mip2_view = mv2;
        self.mip2_bg = mb2;
        self._mip3_tex = m3;
        self.mip3_view = mv3;
        self.mip3_bg = mb3;

        let uniforms = blur_pyramid_uniforms(width, height, w1, h1, w2, h2, w3, h3);
        for (i, u) in uniforms.into_iter().enumerate() {
            gpu.queue
                .write_buffer(&self.blur_param_buffers[i], 0, bytemuck::bytes_of(&u));
        }

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
    ) {
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

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SteelUI Frame Encoder"),
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
                        let _ = begin_canonical_pass(
                            &mut encoder,
                            &self.canonical_view,
                            wgpu::LoadOp::Clear(clear_color.to_wgpu()),
                        );
                        is_first_pass = false;
                    }

                    encoder.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.canonical_tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.backdrop_tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d {
                            width: self.width,
                            height: self.height,
                            depth_or_array_layers: 1,
                        },
                    );

                    run_blur_pass(&mut encoder, &gpu.blur_pipeline, &self.canonical_linear_bg, &self.blur_param_bgs[0], &self.mip1_view);
                    run_blur_pass(&mut encoder, &gpu.blur_pipeline, &self.mip1_bg, &self.blur_param_bgs[1], &self.mip2_view);
                    run_blur_pass(&mut encoder, &gpu.blur_pipeline, &self.mip2_bg, &self.blur_param_bgs[4], &self.mip1_view);

                    {
                        let mut pass = begin_canonical_pass(
                            &mut encoder,
                            &self.canonical_view,
                            wgpu::LoadOp::Load,
                        );
                        apply_scissor(&mut pass, active_scissor, draw_list.scale, self.width, self.height);
                        pass.set_pipeline(&gpu.glass_pipeline);
                        pass.set_bind_group(0, &self.globals_bg, &[]);
                        pass.set_bind_group(1, &self.backdrop_bg, &[]);
                        pass.set_bind_group(2, &self.mip1_bg, &[]);
                        pass.set_vertex_buffer(0, self.glass_buffer.slice(..));
                        pass.draw(0..4, *idx..(*idx + 1));
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
                        let mut pass =
                            begin_canonical_pass(&mut encoder, &self.canonical_view, load);
                        apply_scissor(&mut pass, active_scissor, draw_list.scale, self.width, self.height);
                        while i < draw_list.batches.len() {
                            match &draw_list.batches[i] {
                                DrawBatch::SetScissor(sc) => {
                                    active_scissor = *sc;
                                    apply_scissor(&mut pass, active_scissor, draw_list.scale, self.width, self.height);
                                    i += 1;
                                }
                                DrawBatch::Sdf(range) => {
                                    pass.set_pipeline(&gpu.sdf_pipeline);
                                    pass.set_bind_group(0, &self.globals_bg, &[]);
                                    pass.set_vertex_buffer(0, self.sdf_buffer.slice(..));
                                    pass.draw(0..4, range.clone());
                                    i += 1;
                                }
                                DrawBatch::Image { tex, range } => {
                                    if let Some(uploaded) = gpu.textures.get(tex.0) {
                                        pass.set_pipeline(&gpu.image_pipeline);
                                        pass.set_bind_group(0, &self.globals_bg, &[]);
                                        pass.set_bind_group(1, &uploaded.bind_group, &[]);
                                        pass.set_vertex_buffer(0, self.image_buffer.slice(..));
                                        pass.draw(0..4, range.clone());
                                    }
                                    i += 1;
                                }
                                DrawBatch::Text(range) => {
                                    pass.set_pipeline(&gpu.text_pipeline);
                                    pass.set_bind_group(0, &self.globals_bg, &[]);
                                    pass.set_bind_group(1, &gpu.glyph_atlas.bind_group, &[]);
                                    pass.set_vertex_buffer(0, self.glyph_buffer.slice(..));
                                    pass.draw(0..4, range.clone());
                                    i += 1;
                                }
                                DrawBatch::Glass(_) => break,
                            }
                        }
                    }
                }
            }
        }

        if is_first_pass {
            let _ = begin_canonical_pass(
                &mut encoder,
                &self.canonical_view,
                wgpu::LoadOp::Clear(clear_color.to_wgpu()),
            );
        }

        {
            let blit_pipeline = gpu.blit_pipeline_for(surface_format);
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

            blit_pass.set_pipeline(blit_pipeline);
            blit_pass.set_bind_group(0, &self.canonical_blit_bg, &[]);
            blit_pass.draw(0..3, 0..1);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
    }
}

fn blur_pyramid_uniforms(
    w0: u32,
    h0: u32,
    w1: u32,
    h1: u32,
    w2: u32,
    h2: u32,
    _w3: u32,
    _h3: u32,
) -> [BlurUniform; 5] {
    [
        BlurUniform { texel_size: [1.0 / w0 as f32, 1.0 / h0 as f32], offset: 0.60, _pad: 0.0 },
        BlurUniform { texel_size: [1.0 / w1 as f32, 1.0 / h1 as f32], offset: 0.50, _pad: 0.0 },
        BlurUniform { texel_size: [1.0 / w2 as f32, 1.0 / h2 as f32], offset: 0.00, _pad: 0.0 },
        BlurUniform { texel_size: [1.0 / w2 as f32, 1.0 / h2 as f32], offset: 0.35, _pad: 0.0 },
        BlurUniform { texel_size: [1.0 / w2 as f32, 1.0 / h2 as f32], offset: 0.35, _pad: 0.0 },
    ]
}

fn begin_canonical_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SteelUI Canonical UI Pass"),
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
        format: CANONICAL_FORMAT,
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
        format: CANONICAL_FORMAT,
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

fn create_backdrop_target(gpu: &GpuContext, width: u32, height: u32) -> (wgpu::Texture, wgpu::BindGroup) {
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
        format: CANONICAL_FORMAT,
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
    globals_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("../../shaders/sdf.wgsl"));

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI SDF Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawSdfInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 16, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 32, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 48, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 64, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 80, shader_location: 5, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 84, shader_location: 6, format: wgpu::VertexFormat::Float32x2 },
            wgpu::VertexAttribute { offset: 96, shader_location: 7, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 112, shader_location: 8, format: wgpu::VertexFormat::Float32x3 },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI SDF Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: CANONICAL_FORMAT,
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
    globals_bgl: &wgpu::BindGroupLayout,
    tex_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("../../shaders/textured.wgsl"));

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Image Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl), Some(tex_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawImageInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 16, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 32, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 48, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Image Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: CANONICAL_FORMAT,
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
    globals_bgl: &wgpu::BindGroupLayout,
    atlas_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("../../shaders/text.wgsl"));

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Text Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl), Some(atlas_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawGlyphInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 16, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 32, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Text Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: CANONICAL_FORMAT,
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
    tex_bgl: &wgpu::BindGroupLayout,
    blur_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("../../shaders/blur.wgsl"));

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Blur Pipeline Layout"),
        bind_group_layouts: &[Some(tex_bgl), Some(blur_bgl)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Blur Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: CANONICAL_FORMAT,
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
    globals_bgl: &wgpu::BindGroupLayout,
    tex_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("../../shaders/glass.wgsl"));

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Glass Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl), Some(tex_bgl), Some(tex_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawGlassInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 16, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 32, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 48, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 64, shader_location: 4, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 68, shader_location: 5, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 72, shader_location: 6, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 76, shader_location: 7, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 80, shader_location: 8, format: wgpu::VertexFormat::Float32x4 },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Glass Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: CANONICAL_FORMAT,
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

fn create_tex_resources(
    device: &wgpu::Device,
) -> (
    wgpu::ShaderModule,
    wgpu::BindGroupLayout,
    wgpu::Sampler,
    wgpu::Sampler,
) {
    let shader = device.create_shader_module(wgpu::include_wgsl!("../../shaders/blit.wgsl"));

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

    (shader, bgl, nearest, linear)
}

fn apply_scissor(
    pass: &mut wgpu::RenderPass<'_>,
    scissor: Option<Rect>,
    scale: f32,
    target_width: u32,
    target_height: u32,
) {
    match scissor {
        Some(r) => {
            let x = (r.x * scale).round().max(0.0) as u32;
            let y = (r.y * scale).round().max(0.0) as u32;
            let w = ((r.width * scale).round().max(1.0) as u32)
                .min(target_width.saturating_sub(x))
                .max(1);
            let h = ((r.height * scale).round().max(1.0) as u32)
                .min(target_height.saturating_sub(y))
                .max(1);
            pass.set_scissor_rect(x, y, w, h);
        }
        None => {
            pass.set_scissor_rect(0, 0, target_width, target_height);
        }
    }
}