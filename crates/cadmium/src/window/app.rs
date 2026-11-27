struct WindowContext {
    surface: wgpu::Surface<'static>,
    window: Arc<Window>,
    accesskit: accesskit_winit::Adapter,
    a11y_active: bool,
    config: wgpu::SurfaceConfiguration,
    target: WindowRenderTarget,
    borderless: bool,
    needs_reconfigure: bool,
    clear_color: Color,
    cursor_pos: Option<(f32, f32)>,
    last_drag_click: Option<(Instant, (f32, f32))>,
    last_tick: Instant,
    last_present: Instant,
    last_cursor: CursorIcon,
    is_animating: bool,
    scheduled_redraw: Option<Instant>,
    delegate: Box<dyn WindowDelegate>,
}

impl WindowContext {
    fn configure_surface(&self, gpu: &GpuContext) {
        self.surface.configure(&gpu.device, &self.config);
    }

    fn resize(&mut self, gpu: &GpuContext, width: u32, height: u32) {
        if width == 0 || height == 0 {
            self.config.width = 0;
            self.config.height = 0;
            self.needs_reconfigure = true;
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.needs_reconfigure = false;
        self.configure_surface(gpu);
        self.target.resize(gpu, width, height);
    }

    fn handle_surface_fault(&mut self, gpu: &GpuContext) {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            self.config.width = 0;
            self.config.height = 0;
            self.needs_reconfigure = true;
            return;
        }
        self.resize(gpu, size.width, size.height);
        self.window.request_redraw();
    }

    fn resize_edge(&self, lx: f32, ly: f32) -> Option<ResizeDirection> {
        if !self.borderless || self.window.is_maximized() {
            return None;
        }
        let scale = self.window.scale_factor() as f32;
        let w = self.config.width as f32 / scale;
        let h = self.config.height as f32 / scale;
        let m = 6.0;

        let left = lx <= m;
        let right = lx >= w - m;
        let top = ly <= m;
        let bottom = ly >= h - m;

        match (left, right, top, bottom) {
            (true, _, true, _) => Some(ResizeDirection::NorthWest),
            (_, true, true, _) => Some(ResizeDirection::NorthEast),
            (true, _, _, true) => Some(ResizeDirection::SouthWest),
            (_, true, _, true) => Some(ResizeDirection::SouthEast),
            (true, _, _, _) => Some(ResizeDirection::West),
            (_, true, _, _) => Some(ResizeDirection::East),
            (_, _, true, _) => Some(ResizeDirection::North),
            (_, _, _, true) => Some(ResizeDirection::South),
            _ => None,
        }
    }

    fn render(&mut self, gpu: &mut GpuContext) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }
        if self.needs_reconfigure {
            let size = self.window.inner_size();
            if size.width == 0 || size.height == 0 {
                return;
            }
            self.resize(gpu, size.width, size.height);
        }

        reactive::flush_effects();

        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f32()
            .clamp(0.0001, 0.05);
        self.last_tick = now;
        let animating = self.delegate.tick(dt);

        let (frame, was_suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let size = self.window.inner_size();
                if size.width != self.config.width || size.height != self.config.height {
                    drop(frame);
                    self.handle_surface_fault(gpu);
                    return;
                }
                // Size matches but surface is suboptimal (e.g. Wayland fractional
                // scaling flap).  Present this frame then reconfigure on the *next*
                // frame to avoid a busy-loop of reconfigure → suboptimal → reconfigure.
                (frame, true)
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.handle_surface_fault(gpu);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
            wgpu::CurrentSurfaceTexture::Lost => {
                tracing::warn!("GPU surface lost; attempting reconfigure");
                self.handle_surface_fault(gpu);
                return;
            }
        };

        let scale = self.window.scale_factor() as f32;
        let logical_w = self.config.width as f32 / scale;
        let logical_h = self.config.height as f32 / scale;

        gpu.begin_frame();

        let present_now = Instant::now();
        let frame_dt = present_now.duration_since(self.last_present).as_secs_f32();

        let mut frame_ctx = FrameContext {
            width: logical_w,
            height: logical_h,
            scale,
            frame_dt,
            cursor_pos: self.cursor_pos,
            gpu,
            draw_list: DrawList::new(scale),
        };
        self.delegate.render(&mut frame_ctx);
        let draw_list = frame_ctx.draw_list;

        let surface_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let atlas_overflowed = self.target.render(
            gpu,
            &surface_view,
            self.config.format,
            self.clear_color,
            &draw_list,
        );

        gpu.queue.present(frame);
        if was_suboptimal {
            self.handle_surface_fault(gpu);
        }
        self.last_present = Instant::now();

        if self.a11y_active
            && let Some(tree) = self.delegate.semantic_tree()
        {
            self.accesskit
                .update_if_active(|| accessibility::to_accesskit_update(&tree));
        }

        self.is_animating = animating || atlas_overflowed;
        if self.is_animating {
            self.scheduled_redraw = None;
            self.window.request_redraw();
        } else {
            self.scheduled_redraw = self.delegate.next_redraw_at();
        }
    }
}

/// The application runner: owns the event loop, windows, and shared GPU state.
///
/// Create an app with [`Self::new`], queue windows with [`Self::add_window`],
/// and start the event loop with [`Self::run`]. One [`GpuContext`] is created
/// lazily and shared by every window. The app also drains closures sent through
/// [`Self::dispatcher`] and flushes reactive effects before each frame.
///
/// # Examples
///
/// ```no_run
/// use cadmium::prelude::*;
///
/// let app = App::new();
/// let dispatcher = app.dispatcher();
/// dispatcher.dispatch(|| println!("Runs on the event-loop thread"));
/// let _ = app.run();
/// ```
pub struct App {
    pending_windows: Vec<WindowSpec>,
    gpu: Option<GpuContext>,
    windows: HashMap<WindowId, WindowContext>,
    modifiers: Modifiers,
    initialized: bool,
    async_tx: Sender<Box<dyn FnOnce() + Send>>,
    async_rx: Receiver<Box<dyn FnOnce() + Send>>,
    proxy_slot: Arc<Mutex<Option<EventLoopProxy<SteelEvent>>>>,
    wake_fn: Arc<dyn Fn() + Send + Sync>,
}

impl App {
    /// Creates an empty application and installs the reactive wake callback.
    ///
    /// The callback lets [`crate::reactive`] wake this app's event loop when
    /// effects or signal-driven changes need attention. Windows are not created
    /// until [`Self::add_window`] and [`Self::run`] are called.
    pub fn new() -> Self {
        let (async_tx, async_rx) = mpsc::channel();
        let proxy_slot: Arc<Mutex<Option<EventLoopProxy<SteelEvent>>>> = Arc::new(Mutex::new(None));
        let slot_clone = proxy_slot.clone();
        let wake_fn: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let guard = slot_clone.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(proxy) = &*guard {
                let _ = proxy.send_event(SteelEvent::Wake);
            }
        });
        reactive::set_wake_callback(wake_fn.clone());

        Self {
            pending_windows: Vec::new(),
            gpu: None,
            windows: HashMap::new(),
            modifiers: Modifiers::default(),
            initialized: false,
            async_tx,
            async_rx,
            proxy_slot,
            wake_fn,
        }
    }

    /// Returns a handle for running closures on the application's event-loop
    /// thread.
    ///
    /// The returned [`UiDispatcher`] is cloneable and may be sent to other
    /// threads. [`crate::reactive::UiDispatcher::dispatch`] queues a closure and
    /// wakes the loop; queued closures run on a subsequent event-loop turn and
    /// are followed by a reactive flush and redraw requests. This is the
    /// supported way for background work to update UI state.
    pub fn dispatcher(&self) -> UiDispatcher {
        UiDispatcher::new(self.async_tx.clone(), self.wake_fn.clone())
    }

    /// Queues a window specification to be opened when the event loop starts.
    ///
    /// Takes and returns `self` so calls can be chained. Multiple windows may
    /// be added; they share one GPU context and each keeps its own delegate.
    /// The window is created on [`Self::run`], not by this method.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::prelude::*;
    ///
    /// let app = App::new()
    ///     .add_window(WindowSpec {
    ///         title: "Main".into(),
    ///         width: 480.0,
    ///         height: 320.0,
    ///         min_width: None,
    ///         min_height: None,
    ///         borderless: false,
    ///         clear_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
    ///         delegate: Box::new(WidgetWindow::new(|| Text::new("Main").into_element())),
    ///     })
    ///     .add_window(WindowSpec {
    ///         title: "Palette".into(),
    ///         width: 240.0,
    ///         height: 180.0,
    ///         min_width: None,
    ///         min_height: None,
    ///         borderless: true,
    ///         clear_color: Color::rgba(0.1, 0.1, 0.1, 1.0),
    ///         delegate: Box::new(WidgetWindow::new(|| Text::new("Palette").into_element())),
    ///     });
    /// let _ = app.run();
    /// ```
    pub fn add_window(mut self, spec: WindowSpec) -> Self {
        self.pending_windows.push(spec);
        self
    }

    /// Creates the queued windows and runs the event loop until all are closed.
    ///
    /// Returns [`Ok`] when the loop exits normally and a [`CadmiumError`] if the
    /// event loop cannot be created or run. A failure while opening an
    /// individual window is logged and causes the loop to exit. Closing the
    /// last window also exits the loop. This method consumes the app; build a
    /// new one to start again.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::prelude::*;
    ///
    /// # fn main() -> Result<(), CadmiumError> {
    /// App::new()
    ///     .add_window(WindowSpec {
    ///         title: "Counter".into(),
    ///         width: 320.0,
    ///         height: 240.0,
    ///         min_width: None,
    ///         min_height: None,
    ///         borderless: false,
    ///         clear_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
    ///         delegate: Box::new(WidgetWindow::new(|| Text::new("Count").into_element())),
    ///     })
    ///     .run()
    /// # }
    /// ```
    pub fn run(mut self) -> Result<(), CadmiumError> {
        let event_loop = EventLoop::<SteelEvent>::with_user_event().build()?;
        let proxy = event_loop.create_proxy();
        *self.proxy_slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(proxy);
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    fn drain_async_tasks(&mut self) {
        let mut ran = false;
        while let Ok(task) = self.async_rx.try_recv() {
            task();
            ran = true;
        }
        if ran {
            reactive::flush_effects();
            for win in self.windows.values() {
                win.window.request_redraw();
            }
        }
    }

    fn spawn_window(
        event_loop: &ActiveEventLoop,
        proxy: Option<EventLoopProxy<SteelEvent>>,
        gpu: &mut Option<GpuContext>,
        mut spec: WindowSpec,
    ) -> Result<(WindowId, WindowContext), CadmiumError> {
        let mut attrs = Window::default_attributes()
            .with_title(spec.title)
            .with_decorations(!spec.borderless)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(spec.width, spec.height));

        if let (Some(min_w), Some(min_h)) = (spec.min_width, spec.min_height) {
            attrs = attrs.with_min_inner_size(LogicalSize::new(min_w, min_h));
        }

        #[cfg(target_os = "windows")]
        if spec.borderless {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_undecorated_shadow(true);
        }

        let window = Arc::new(event_loop.create_window(attrs)?);
        let id = window.id();

        let proxy = proxy.ok_or_else(|| {
            CadmiumError::Platform("EventLoopProxy unavailable during window spawn".into())
        })?;
        let accesskit = accesskit_winit::Adapter::with_event_loop_proxy(event_loop, &window, proxy);
        window.set_visible(true);

        let (surface, created_gpu) = if let Some(ctx) = gpu.as_ref() {
            (ctx.instance.create_surface(window.clone())?, None)
        } else {
            let make_instance = |backends: wgpu::Backends| {
                let mut desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                    event_loop.owned_display_handle(),
                ));
                if cfg!(not(debug_assertions)) {
                    desc.flags = wgpu::InstanceFlags::empty();
                }
                desc.backends = backends;
                #[cfg(target_os = "windows")]
                {
                    desc.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
                }
                wgpu::Instance::new(desc)
            };

            #[cfg(target_os = "windows")]
            let primary_backends = wgpu::Backends::DX12;
            #[cfg(not(target_os = "windows"))]
            let primary_backends = wgpu::Backends::PRIMARY;

            let instance = make_instance(primary_backends);
            let surface = instance.create_surface(window.clone())?;
            match pollster::block_on(GpuContext::new(instance, Some(&surface))) {
                Ok(ctx) => (surface, Some(ctx)),
                Err(primary_err) => {
                    #[cfg(target_os = "windows")]
                    {
                        tracing::warn!("DX12 init failed ({primary_err}); falling back to Vulkan");
                        drop(surface);
                        let vk_instance = make_instance(wgpu::Backends::VULKAN);
                        let vk_surface = vk_instance.create_surface(window.clone())?;
                        let ctx =
                            pollster::block_on(GpuContext::new(vk_instance, Some(&vk_surface)))?;
                        (vk_surface, Some(ctx))
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        return Err(primary_err);
                    }
                }
            }
        };

        if let Some(ctx) = created_gpu {
            *gpu = Some(ctx);
        }

        let ctx = gpu.as_mut().expect("GpuContext initialized");
        spec.delegate.init(&mut ctx.text_engine);
        spec.delegate.init_gpu(ctx);

        let caps = surface.get_capabilities(&ctx.adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| {
                CadmiumError::Platform("Surface reported no supported texture formats".into())
            })?;

        let alpha_mode = caps.alpha_modes.first().copied().ok_or_else(|| {
            CadmiumError::Platform("Surface reported no supported alpha modes".into())
        })?;

        let present_mode = [
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::FifoRelaxed,
            wgpu::PresentMode::Fifo,
        ]
        .into_iter()
        .find(|m| caps.present_modes.contains(m))
        .unwrap_or(wgpu::PresentMode::Fifo);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&ctx.device, &config);

        let target = WindowRenderTarget::new(ctx, config.width, config.height);
        window.request_redraw();

        Ok((
            id,
            WindowContext {
                surface,
                window,
                accesskit,
                a11y_active: false,
                config,
                target,
                borderless: spec.borderless,
                needs_reconfigure: false,
                clear_color: spec.clear_color,
                cursor_pos: None,
                last_drag_click: None,
                last_tick: Instant::now(),
                last_present: Instant::now(),
                last_cursor: CursorIcon::Default,
                is_animating: false,
                scheduled_redraw: None,
                delegate: spec.delegate,
            },
        ))
    }
}

