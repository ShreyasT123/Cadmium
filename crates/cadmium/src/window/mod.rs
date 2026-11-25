use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{CursorIcon, ResizeDirection, Window, WindowId},
};

use crate::accessibility::{self, A11yAction, SemanticTree};
use crate::core::{Color, SteelError};
use crate::events::{Key, Modifiers};
use crate::reactive::{self, UiDispatcher};
use crate::renderer::{DrawList, GlassQuad, GpuContext, ImageQuad, SdfQuad, WindowRenderTarget};
use crate::text::TextEngine;

#[derive(Debug)]
enum SteelEvent {
    Wake,
    AccessKit(accesskit_winit::Event),
}

impl From<accesskit_winit::Event> for SteelEvent {
    fn from(inner: accesskit_winit::Event) -> Self {
        Self::AccessKit(inner)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Default,
    Pointer,
    Grab,
    Grabbing,
    ColResize,
    RowResize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    None,
    Close,
    Minimize,
    ToggleMaximize,
    RequestRedrawAll,
}

pub struct FrameContext<'a> {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub frame_dt: f32,
    pub cursor_pos: Option<(f32, f32)>,
    gpu: &'a mut GpuContext,
    draw_list: DrawList,
}

impl<'a> FrameContext<'a> {
    pub(crate) fn gpu_mut(&mut self) -> &mut GpuContext {
        self.gpu
    }

    pub fn text_engine(&mut self) -> &mut TextEngine {
        &mut self.gpu.text_engine
    }

    pub fn push_sdf(&mut self, quad: SdfQuad) {
        self.draw_list.push_sdf(quad);
    }

    pub fn push_image(&mut self, img: ImageQuad) {
        self.draw_list.push_image(img);
    }

    pub fn push_glass(&mut self, glass: GlassQuad) {
        self.draw_list.push_glass(glass);
    }

    pub fn push_stroke_path(
        &mut self,
        path: &crate::renderer::LyonPath,
        color: crate::core::Color,
        width: f32,
    ) {
        self.draw_list.push_stroke_path(path, color, width);
    }
    pub fn push_text_colored(
        &mut self,
        block: &crate::text::TextBlock,
        logical_x: f32,
        logical_y: f32,
        color: Color,
    ) {
        self.draw_list
            .push_text_colored(self.gpu, block, logical_x, logical_y, Some(color));
    }

    pub fn push_clip(&mut self, rect: crate::core::Rect) {
        self.draw_list.push_clip(rect);
    }

    pub fn pop_clip(&mut self) {
        self.draw_list.pop_clip();
    }

    pub fn push_text(&mut self, block: &crate::text::TextBlock, logical_x: f32, logical_y: f32) {
        self.draw_list
            .push_text(self.gpu, block, logical_x, logical_y);
    }
}

pub trait WindowDelegate: 'static {
    fn init(&mut self, _text: &mut TextEngine) {}
    fn init_gpu(&mut self, _gpu: &mut GpuContext) {}
    fn tick(&mut self, _dt: f32) -> bool {
        false
    }
    fn next_redraw_at(&self) -> Option<Instant> {
        None
    }
    fn on_pointer_move(&mut self, _x: f32, _y: f32) -> bool {
        false
    }
    fn on_pointer_down(&mut self, _x: f32, _y: f32) -> (bool, WindowCommand) {
        (false, WindowCommand::None)
    }
    fn on_secondary_down(&mut self, _x: f32, _y: f32) -> bool {
        false
    }
    fn on_pointer_up(&mut self) -> bool {
        false
    }

    fn on_scroll(&mut self, _delta_x: f32, _delta_y: f32) -> bool {
        false
    }
    fn on_key(&mut self, _key: &Key, _text: Option<&str>, _modifiers: &Modifiers) -> bool {
        false
    }
    fn is_drag_region(&self, _x: f32, _y: f32) -> bool {
        false
    }
    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Default
    }
    fn set_a11y_active(&mut self, _active: bool) {}
    fn semantic_tree(&self) -> Option<SemanticTree> {
        None
    }
    fn on_semantic_action(&mut self, _action: A11yAction) -> bool {
        false
    }
    fn render(&mut self, ctx: &mut FrameContext<'_>);
    fn on_window_focus(&mut self, _focused: bool) -> bool {
        false
    }
}

pub struct WindowSpec {
    pub title: String,
    pub width: f64,
    pub height: f64,
    pub borderless: bool,
    pub clear_color: Color,
    pub delegate: Box<dyn WindowDelegate>,
}

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

    pub fn dispatcher(&self) -> UiDispatcher {
        UiDispatcher::new(self.async_tx.clone(), self.wake_fn.clone())
    }

    pub fn add_window(mut self, spec: WindowSpec) -> Self {
        self.pending_windows.push(spec);
        self
    }

    pub fn run(mut self) -> Result<(), SteelError> {
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
    ) -> Result<(WindowId, WindowContext), SteelError> {
        let attrs = Window::default_attributes()
            .with_title(spec.title)
            .with_decorations(!spec.borderless)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(spec.width, spec.height));

        let window = Arc::new(event_loop.create_window(attrs)?);
        let id = window.id();

        let proxy = proxy.ok_or_else(|| {
            SteelError::Platform("EventLoopProxy unavailable during window spawn".into())
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
                SteelError::Platform("Surface reported no supported texture formats".into())
            })?;

        let alpha_mode = caps.alpha_modes.first().copied().ok_or_else(|| {
            SteelError::Platform("Surface reported no supported alpha modes".into())
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

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl ApplicationHandler<SteelEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        let proxy = self
            .proxy_slot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let specs = std::mem::take(&mut self.pending_windows);
        for spec in specs {
            match Self::spawn_window(event_loop, proxy.clone(), &mut self.gpu, spec) {
                Ok((id, win_ctx)) => {
                    self.windows.insert(id, win_ctx);
                }
                Err(err) => {
                    tracing::error!("Failed to open window: {err}");
                    event_loop.exit();
                    return;
                }
            }
        }
        self.drain_async_tasks();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: SteelEvent) {
        match event {
            SteelEvent::Wake => self.drain_async_tasks(),
            SteelEvent::AccessKit(ak_event) => {
                if let Some(win) = self.windows.get_mut(&ak_event.window_id) {
                    match ak_event.window_event {
                        accesskit_winit::WindowEvent::InitialTreeRequested => {
                            win.a11y_active = true;
                            win.delegate.set_a11y_active(true);
                            win.window.request_redraw();
                        }
                        accesskit_winit::WindowEvent::ActionRequested(req) => {
                            if let Some(action) = accessibility::action_from_request(&req)
                                && win.delegate.on_semantic_action(action)
                            {
                                reactive::flush_effects();
                                win.window.request_redraw();
                            }
                        }
                        accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                            win.a11y_active = false;
                            win.delegate.set_a11y_active(false);
                        }
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let mut earliest: Option<Instant> = None;

        for win in self.windows.values_mut() {
            if let Some(target_t) = win.scheduled_redraw {
                if now >= target_t {
                    win.scheduled_redraw = None;
                    win.window.request_redraw();
                } else {
                    earliest = Some(match earliest {
                        Some(e) => e.min(target_t),
                        None => target_t,
                    });
                }
            }
        }

        match earliest {
            Some(t) => event_loop.set_control_flow(ControlFlow::WaitUntil(t)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(win) = self.windows.get_mut(&id) {
            win.accesskit.process_event(&win.window, &event);
        }

        match event {
            WindowEvent::ModifiersChanged(mods) => {
                let state = mods.state();
                self.modifiers = Modifiers {
                    shift: state.shift_key(),
                    ctrl: state.control_key(),
                    alt: state.alt_key(),
                    super_key: state.super_key(),
                };
            }
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.modifiers = Modifiers::default();
                }
                if let Some(win) = self.windows.get_mut(&id)
                    && win.delegate.on_window_focus(focused)
                {
                    reactive::flush_effects();
                    win.window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (x * 40.0, y * 40.0),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                };
                if let Some(win) = self.windows.get_mut(&id)
                    && win.delegate.on_scroll(dx, dy)
                {
                    reactive::flush_effects();
                    win.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                if event.state.is_pressed() && !is_synthetic {
                    let steel_key = from_winit_key(&event.logical_key);
                    let text = event.text.as_deref();
                    if let Some(win) = self.windows.get_mut(&id)
                        && win.delegate.on_key(&steel_key, text, &self.modifiers)
                    {
                        reactive::flush_effects();
                        win.window.request_redraw();
                    }
                }
            }
            WindowEvent::CloseRequested => {
                self.windows.remove(&id);
                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let (Some(gpu), Some(win)) = (&self.gpu, self.windows.get_mut(&id)) {
                    win.resize(gpu, size.width, size.height);
                    if size.width > 0 && size.height > 0 {
                        win.window.request_redraw();
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let (Some(gpu), Some(win)) = (&self.gpu, self.windows.get_mut(&id)) {
                    win.handle_surface_fault(gpu);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(win) = self.windows.get_mut(&id) {
                    let scale = win.window.scale_factor() as f32;
                    let lx = position.x as f32 / scale;
                    let ly = position.y as f32 / scale;
                    win.cursor_pos = Some((lx, ly));

                    let cursor = match win.resize_edge(lx, ly) {
                        Some(ResizeDirection::North | ResizeDirection::South) => {
                            CursorIcon::NsResize
                        }
                        Some(ResizeDirection::East | ResizeDirection::West) => CursorIcon::EwResize,
                        Some(ResizeDirection::NorthWest | ResizeDirection::SouthEast) => {
                            CursorIcon::NwseResize
                        }
                        Some(ResizeDirection::NorthEast | ResizeDirection::SouthWest) => {
                            CursorIcon::NeswResize
                        }
                        None => match win.delegate.cursor_style() {
                            CursorStyle::Default => CursorIcon::Default,
                            CursorStyle::Pointer => CursorIcon::Pointer,
                            CursorStyle::Grab => CursorIcon::Grab,
                            CursorStyle::Grabbing => CursorIcon::Grabbing,
                            CursorStyle::ColResize => CursorIcon::EwResize,
                            CursorStyle::RowResize => CursorIcon::NsResize,
                        },
                    };
                    if win.last_cursor != cursor {
                        win.last_cursor = cursor;
                        win.window.set_cursor(cursor);
                    }

                    if win.delegate.on_pointer_move(lx, ly) {
                        reactive::flush_effects();
                        win.window.request_redraw();
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(win) = self.windows.get_mut(&id) {
                    win.cursor_pos = None;
                    if win.last_cursor != CursorIcon::Default {
                        win.last_cursor = CursorIcon::Default;
                        win.window.set_cursor(CursorIcon::Default);
                    }
                    if win.delegate.on_pointer_move(-1.0, -1.0) {
                        reactive::flush_effects();
                        win.window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if let Some(win) = self.windows.get_mut(&id)
                    && let Some((lx, ly)) = win.cursor_pos
                    && win.delegate.on_secondary_down(lx, ly)
                {
                    reactive::flush_effects();
                    win.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let mut command = WindowCommand::None;
                let mut redraw = false;

                if let Some(win) = self.windows.get_mut(&id)
                    && let Some((lx, ly)) = win.cursor_pos
                {
                    if let Some(dir) = win.resize_edge(lx, ly) {
                        let _ = win.window.drag_resize_window(dir);
                        return;
                    }
                    let (handled, cmd) = win.delegate.on_pointer_down(lx, ly);
                    command = cmd;
                    redraw = handled;
                    if !handled && cmd == WindowCommand::None && win.delegate.is_drag_region(lx, ly)
                    {
                        win.window.request_redraw();
                        let _ = win.window.drag_window();
                        return;
                    }
                }

                reactive::flush_effects();

                match command {
                    WindowCommand::Close => {
                        self.windows.remove(&id);
                        if self.windows.is_empty() {
                            event_loop.exit();
                        }
                    }
                    WindowCommand::Minimize => {
                        if let Some(win) = self.windows.get(&id) {
                            win.window.set_minimized(true);
                        }
                    }
                    WindowCommand::ToggleMaximize => {
                        if let Some(win) = self.windows.get(&id) {
                            win.window.set_maximized(!win.window.is_maximized());
                        }
                    }
                    WindowCommand::RequestRedrawAll => {
                        for win in self.windows.values() {
                            win.window.request_redraw();
                        }
                    }
                    WindowCommand::None => {
                        if redraw && let Some(win) = self.windows.get_mut(&id) {
                            win.window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(win) = self.windows.get_mut(&id)
                    && win.delegate.on_pointer_up()
                {
                    reactive::flush_effects();
                    win.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let (Some(gpu), Some(win)) = (&mut self.gpu, self.windows.get_mut(&id)) {
                    win.render(gpu);
                }
            }
            _ => {}
        }
    }
}

fn from_winit_key(key: &winit::keyboard::Key) -> Key {
    use winit::keyboard::{Key as WK, NamedKey};
    match key {
        WK::Character(s) => Key::Character(s.to_string()),
        WK::Named(NamedKey::Backspace) => Key::Backspace,
        WK::Named(NamedKey::Delete) => Key::Delete,
        WK::Named(NamedKey::Enter) => Key::Enter,
        WK::Named(NamedKey::Tab) => Key::Tab,
        WK::Named(NamedKey::Escape) => Key::Escape,
        WK::Named(NamedKey::Space) => Key::Space,
        WK::Named(NamedKey::ArrowLeft) => Key::ArrowLeft,
        WK::Named(NamedKey::ArrowRight) => Key::ArrowRight,
        WK::Named(NamedKey::ArrowUp) => Key::ArrowUp,
        WK::Named(NamedKey::ArrowDown) => Key::ArrowDown,
        WK::Named(NamedKey::Home) => Key::Home,
        WK::Named(NamedKey::End) => Key::End,
        _ => Key::Unknown,
    }
}
