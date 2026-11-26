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
                        let now = Instant::now();
                        let is_double = win.last_drag_click.is_some_and(|(t, (px, py))| {
                            now.duration_since(t) < Duration::from_millis(400)
                                && (lx - px).hypot(ly - py) < 5.0
                        });
                        if is_double {
                            win.last_drag_click = None;
                            win.window.set_maximized(!win.window.is_maximized());
                            return;
                        }
                        win.last_drag_click = Some((now, (lx, ly)));
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

