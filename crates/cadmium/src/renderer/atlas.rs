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
                #[allow(clippy::chunks_exact_to_as_chunks)]
                // MSRV 1.85: as_chunks requires Rust 1.88
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

