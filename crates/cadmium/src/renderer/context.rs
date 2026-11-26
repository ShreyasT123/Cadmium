struct FormatPipelines {
    sdf: wgpu::RenderPipeline,
    path: wgpu::RenderPipeline,
    text: wgpu::RenderPipeline,
    image: Option<wgpu::RenderPipeline>,
    blur: Option<wgpu::RenderPipeline>,
    glass: Option<wgpu::RenderPipeline>,
    blit: Option<wgpu::RenderPipeline>,
}

/// Owned GPU resources shared by all windows of an application.
///
/// A context is initialized once by [`crate::window::App`] and provides the
/// device, queue, text engine, glyph atlas, upload texture registry, and
/// render pipelines needed to execute a [`DrawList`]. Pipelines are created
/// lazily, cached per target texture format, and reused across frames. The
/// public `wgpu` fields are exposed for advanced callers, such as a
/// [`crate::widgets::CustomGpuPass`], that need direct device access.
pub struct GpuContext {
    /// The `wgpu` instance used to request the adapter.
    pub instance: wgpu::Instance,
    /// The selected low-power adapter, compatible with the surface passed to
    /// [`Self::new`] when one was supplied.
    pub adapter: wgpu::Adapter,
    /// The logical device. It is created with default limits, no optional
    /// features and a `MemoryUsage` memory hint.
    pub device: wgpu::Device,
    /// Queue used for texture/buffer uploads and command submission.
    pub queue: wgpu::Queue,

    /// Text shaping and rasterization engine used by [`DrawList::push_text`].
    ///
    /// During a frame this is normally reached through
    /// [`crate::window::FrameContext::text_engine`].
    pub text_engine: TextEngine,
    glyph_atlas: GlyphAtlas,
    textures: SlotMap<TextureHandle, UploadedTexture>,

    globals_bgl: wgpu::BindGroupLayout,
    tex_bgl: wgpu::BindGroupLayout,
    blur_bgl: wgpu::BindGroupLayout,
    nearest_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,

    sdf_shader: wgpu::ShaderModule,
    path_shader: wgpu::ShaderModule,
    text_shader: wgpu::ShaderModule,
    image_shader: Option<wgpu::ShaderModule>,
    blur_shader: Option<wgpu::ShaderModule>,
    glass_shader: Option<wgpu::ShaderModule>,
    blit_shader: Option<wgpu::ShaderModule>,

    pipelines: HashMap<wgpu::TextureFormat, FormatPipelines>,
}

impl GpuContext {
    /// Initializes GPU resources using `instance`.
    ///
    /// Requests a low-power adapter compatible with `compatible_surface`
    /// when supplied (as it is for a window-backed app), then a device with
    /// default limits and no optional features. Core shaders, bind-group
    /// layouts, samplers, and a 256×256 [`ATLAS_FORMAT`] glyph atlas are
    /// created eagerly; image, blur, glass and blit shaders and the
    /// format-specific pipelines are created on first use.
    ///
    /// Returns [`CadmiumError::RequestAdapter`] if no compatible adapter is
    /// available, or [`CadmiumError::RequestDevice`] if the device request
    /// fails. Both errors wrap the underlying `wgpu` error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn init() -> Result<(), Box<dyn std::error::Error>> {
    /// use cadmium::renderer::GpuContext;
    ///
    /// // Pass `Some(&surface)` when rendering to a window so that the
    /// // adapter is guaranteed to support that surface.
    /// let gpu = GpuContext::new(wgpu::Instance::default(), None).await?;
    /// println!("adapter: {}", gpu.adapter.get_info().name);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, CadmiumError> {
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
        let path_shader =
            device.create_shader_module(wgpu::include_wgsl!("../../shaders/path.wgsl"));
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
            textures: SlotMap::with_key(),
            globals_bgl,
            tex_bgl,
            blur_bgl,
            nearest_sampler,
            linear_sampler,
            sdf_shader,
            path_shader,
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

    /// Uploads a decoded RGBA image to a new GPU texture.
    ///
    /// Creates a single-mip [`ATLAS_FORMAT`] texture sized from
    /// [`DecodedImage::width`] and [`DecodedImage::height`] (clamped to at
    /// least one pixel) and writes `img.rgba` to it immediately through the
    /// queue. The texture is sampled with the linear sampler and clamped to
    /// its edges.
    ///
    /// Returns a handle for use in [`ImageQuad`]. The texture lives until
    /// [`Self::release_texture`] is called. `img.rgba` is expected to hold
    /// straight RGBA8 pixels in row-major order with four bytes per pixel.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::assets::load_image_file;
    /// use cadmium::renderer::{GpuContext, TextureHandle};
    /// use cadmium::CadmiumError;
    ///
    /// # fn load(gpu: &mut GpuContext) -> Result<TextureHandle, CadmiumError> {
    /// let image = load_image_file("icon.png")?;
    /// let handle = gpu.upload_image(&image);
    /// # Ok(handle)
    /// # }
    /// ```
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

    /// Allocates an offscreen render-target texture for
    /// [`crate::widgets::CustomGpuSurface`] and registers it for [`ImageQuad`].
    ///
    /// `width` and `height` are texture dimensions in pixels and are clamped
    /// to at least one. `format` should be a renderable, filterable color
    /// format such as [`ATLAS_FORMAT`] or the surface's preferred sRGB
    /// format: the returned texture has `RENDER_ATTACHMENT` and
    /// `TEXTURE_BINDING` usage and is bound with a filtering sampler, so
    /// depth formats and non-filterable formats cannot be sampled by an
    /// [`ImageQuad`]. The texture has no mip levels and no `COPY_SRC` usage.
    ///
    /// Returns the default full-texture view to use as a render attachment
    /// in a custom pass, together with a [`TextureHandle`] that can be
    /// placed in an [`ImageQuad`] to composite the result into the UI. Call
    /// [`Self::release_texture`] when the target is no longer needed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::renderer::{ATLAS_FORMAT, GpuContext, ImageQuad, TextureHandle};
    /// use cadmium::Rect;
    ///
    /// # fn make_target(gpu: &mut GpuContext) -> (wgpu::TextureView, TextureHandle) {
    /// let (view, handle) = gpu.create_render_texture(512, 512, ATLAS_FORMAT);
    /// let quad = ImageQuad::new(handle, Rect::new(0.0, 0.0, 512.0, 512.0));
    /// # let _ = quad;
    /// # (view, handle)
    /// # }
    /// ```
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
            label: Some("Cadmium Image BG"),
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
        self.textures.insert(UploadedTexture {
            _texture: texture,
            bind_group,
        })
    }

    /// Frees the GPU texture associated with `handle`.
    ///
    /// Drops the texture and its bind group, invalidating the handle. This is
    /// idempotent: an already-released or otherwise stale handle is ignored.
    /// Any [`ImageQuad`] still referencing the handle will be skipped at
    /// render time rather than causing an error. Do not release a texture
    /// that a custom render pass still intends to use.
    pub fn release_texture(&mut self, handle: TextureHandle) {
        self.textures.remove(handle);
    }

    fn ensure_base_pipelines(&mut self, format: wgpu::TextureFormat) {
        self.pipelines
            .entry(format)
            .or_insert_with(|| FormatPipelines {
                sdf: create_sdf_pipeline(&self.device, &self.sdf_shader, &self.globals_bgl, format),
                path: create_path_pipeline(
                    &self.device,
                    &self.path_shader,
                    &self.globals_bgl,
                    format,
                ),
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

