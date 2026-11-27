struct OverlayRenderRecord {
    source_id: WidgetId,
    trigger_bounds: Rect,
    selected: usize,
    item_count: usize,
}

struct ContextMenuState {
    owner: WidgetId,
    anchor: (f32, f32),
    menu_rect: Rect,
    hovered_idx: Option<usize>,
    labels: Vec<Option<(TextBlock, TextBlock)>>,
}

struct PaintCtx<'a, 'b> {
    ctx: &'a mut FrameContext<'b>,
    theme: &'a MacTheme,
    scale: f32,
    time_secs: f32,
    caret_visible: bool,
    hovered: Option<WidgetId>,
    focused: Option<WidgetId>,
    overlays: &'a mut Vec<OverlayRenderRecord>,
    a11y_records: Option<&'a mut Vec<A11yNodeRecord>>,
    selection_scratch: &'a mut Vec<(f32, f32, f32)>,
    decoded_tex_cache: &'a mut HashMap<usize, (TextureHandle, usize)>,
}

/// A [`WindowDelegate`] that drives a widget tree inside a native window.
///
/// The build closure is called once during window initialization; its
/// [`Element`] is mounted into the delegate's [`WidgetArena`]. Subsequent
/// updates come from reactive signals and the effect/notification pipeline,
/// not from rebuilding the closure. The delegate handles pointer input,
/// keyboard focus, tooltips, context menus, dropdown overlays, accessibility,
/// and painting of the mounted tree.
///
/// Type parameter `F` is the build closure, constrained to
/// `FnMut() -> Element + 'static` for the inherent methods below.
///
/// [`WindowDelegate`]: crate::window::WindowDelegate
///
/// # Examples
///
/// ```no_run
/// use cadmium::prelude::*;
///
/// let count = create_signal(0usize);
/// let delegate = WidgetWindow::new(move || {
///     Column::new()
///         .gap(8.0)
///         .child(Text::new(format!("Count: {}", count.get())).size(18.0))
///         .child(Button::new("Increment").on_click(move || count.update(|n| *n += 1)))
///         .into_element()
/// })
/// .show_fps(true);
///
/// let _ = App::new()
///     .add_window(WindowSpec {
///         title: "Counter".into(),
///         width: 320.0,
///         height: 240.0,
///         min_width: None,
///         min_height: None,
///         borderless: false,
///         clear_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
///         delegate: Box::new(delegate),
///     })
///     .run();
/// ```
pub struct WidgetWindow<F> {
    build_fn: F,
    theme_signal: Option<Signal<ThemeMode>>,
    viewport_signal: Option<Signal<(f32, f32)>>,
    arena: WidgetArena,
    profiler: FrameProfiler,
    last_size: (f32, f32),
    hovered: Option<WidgetId>,
    captured: Option<WidgetId>,
    open_dropdowns: Vec<(WidgetId, Rect, usize)>,
    hovered_dropdown_row: Option<(WidgetId, usize)>,
    context_menu: Option<ContextMenuState>,
    tooltip_id: Option<WidgetId>,
    tooltip_anchor: Rect,
    tooltip_show_at: Option<Instant>,
    tooltip_visible: bool,
    tooltip_cached_id: Option<WidgetId>,
    tooltip_block: Option<TextBlock>,
    caret_visible: bool,
    next_blink_at: Option<Instant>,
    start_instant: Instant,
    show_fps: bool,
    fps_block: Option<TextBlock>,
    was_animating: bool,
    path_draw_fn: Option<PathDrawFn>,
    last_fps_update: Instant,
    last_cursor: (f32, f32),
    a11y_active: bool,
    cached_semantic_tree: Option<SemanticTree>,
}

impl<F> WidgetWindow<F>
where
    F: FnMut() -> Element + 'static,
{
    /// Creates a window delegate that mounts the tree returned by `build_fn`.
    ///
    /// The closure is invoked once when the window initializes and must produce
    /// a complete [`Element`] tree. It is stored by value and must be
    /// `'static` because the delegate owns it for the window's lifetime.
    pub fn new(build_fn: F) -> Self {
        Self {
            build_fn,
            theme_signal: None,
            viewport_signal: None,
            arena: WidgetArena::new(),
            profiler: FrameProfiler::default(),
            last_size: (0.0, 0.0),
            hovered: None,
            captured: None,
            open_dropdowns: Vec::new(),
            hovered_dropdown_row: None,
            context_menu: None,
            tooltip_id: None,
            tooltip_anchor: Rect::default(),
            tooltip_show_at: None,
            tooltip_visible: false,
            tooltip_cached_id: None,
            tooltip_block: None,
            caret_visible: true,
            next_blink_at: None,
            start_instant: Instant::now(),
            show_fps: false,
            fps_block: None,
            was_animating: false,
            path_draw_fn: None,
            last_fps_update: Instant::now(),
            last_cursor: (0.0, 0.0),
            a11y_active: false,
            cached_semantic_tree: None,
        }
    }

    /// Binds the window's theme mode to a reactive signal.
    ///
    /// The signal is sampled each frame during painting; a change therefore
    /// takes effect on the next redraw without rebuilding the widget tree.
    /// Without this call the delegate uses [`ThemeMode::Light`].
    pub fn with_theme(mut self, theme_signal: Signal<ThemeMode>) -> Self {
        self.theme_signal = Some(theme_signal);
        self
    }

    /// Binds the window's logical viewport size to a reactive signal.
    ///
    /// The signal is updated whenever the window's logical `(width, height)`
    /// changes, allowing widgets to derive responsive breakpoints with
    /// [`crate::Breakpoint::from_width`].
    pub fn with_viewport(mut self, viewport_signal: Signal<(f32, f32)>) -> Self {
        self.viewport_signal = Some(viewport_signal);
        self
    }

    /// Enables or disables the built-in frame statistics overlay.
    ///
    /// When enabled, a small label is drawn over the widget tree showing frame
    /// time and, while animations are active, presented FPS. It is off by
    /// default.
    pub fn show_fps(mut self, show: bool) -> Self {
        self.show_fps = show;
        self
    }

    /// Registers a callback for drawing debug geometry over the window.
    ///
    /// The callback receives the active [`FrameContext`] and runs before the
    /// mounted widget tree is painted, so widgets can be drawn on top of it.
    /// Use this for path overlays and other visual debugging aids.
    pub fn draw_paths(mut self, f: impl FnMut(&mut FrameContext<'_>) + 'static) -> Self {
        self.path_draw_fn = Some(Box::new(f));
        self
    }

    /// Returns the frame profiler collecting timing metrics for this window.
    ///
    /// The returned reference can be used to inspect layout, paint, and frame
    /// cadence measurements after frames have been rendered.
    pub fn profiler(&self) -> &FrameProfiler {
        &self.profiler
    }

    fn hit_test(&self, x: f32, y: f32) -> Option<WidgetId> {
        self.arena
            .root
            .and_then(|root| hit_test_node(root, &self.arena.nodes, x, y, (0.0, 0.0)))
    }

    fn update_slider(&mut self, id: WidgetId, x: f32) -> bool {
        if let Some(node) = self.arena.nodes.get(id)
            && let WidgetPayload::Slider { signal, min, max } = node.payload
        {
            let t = ((x - node.bounds.x) / node.bounds.width.max(1.0)).clamp(0.0, 1.0);
            let val = min + (max - min) * t;
            signal.set(val);
            return true;
        }
        false
    }

    fn clear_tooltip(&mut self) -> bool {
        let was_visible = self.tooltip_visible;
        self.tooltip_id = None;
        self.tooltip_show_at = None;
        self.tooltip_visible = false;
        was_visible
    }

    fn reset_caret_blink(&mut self) {
        let is_text_focused = self
            .arena
            .focus
            .focused
            .and_then(|id| self.arena.nodes.get(id))
            .is_some_and(|n| {
                matches!(
                    n.payload,
                    WidgetPayload::TextInput { .. } | WidgetPayload::TextArea { .. }
                )
            });

        if is_text_focused {
            self.caret_visible = true;
            self.next_blink_at = Some(Instant::now() + CARET_BLINK_INTERVAL);
        } else {
            self.caret_visible = false;
            self.next_blink_at = None;
        }
    }
}

impl<F> WindowDelegate for WidgetWindow<F>
where
    F: FnMut() -> Element + 'static,
{
    fn init(&mut self, text: &mut TextEngine) {
        self.hovered = None;
        self.captured = None;
        self.open_dropdowns.clear();
        self.hovered_dropdown_row = None;
        self.context_menu = None;
        self.tooltip_id = None;
        self.tooltip_show_at = None;
        self.tooltip_visible = false;
        self.tooltip_cached_id = None;
        self.tooltip_block = None;
        self.caret_visible = false;
        self.next_blink_at = None;
        self.cached_semantic_tree = None;

        let root_elem = (self.build_fn)();
        self.arena.mount(root_elem, text);
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        for &sid in &self.arena.scroll_ids {
            if let Some(node) = self.arena.nodes.get_mut(sid)
                && let WidgetPayload::ScrollView { scroll_y, .. } = &mut node.payload
            {
                animating |= scroll_y.step(dt);
            }
        }
        for &tid in &self.arena.tab_ids {
            if let Some(node) = self.arena.nodes.get_mut(tid)
                && let WidgetPayload::Tabs {
                    signal,
                    pill_spring,
                    ..
                } = &mut node.payload
            {
                pill_spring.set_target(signal.try_get().unwrap_or(0) as f32);
                animating |= pill_spring.step(dt);
            }
        }
        for &cid in &self.arena.custom_surface_ids {
            if let Some(node) = self.arena.nodes.get(cid)
                && let WidgetPayload::CustomGpuSurface {
                    animate_signal: Some(sig),
                    ..
                } = &node.payload
                && sig.try_get().unwrap_or(false)
            {
                animating = true;
            }
        }
        for &vid in &self.arena.virtual_list_ids {
            if let Some(node) = self.arena.nodes.get(vid)
                && let WidgetPayload::VirtualizedList { state } = &node.payload
            {
                animating |= state.borrow_mut().scroll_y.step(dt);
            }
        }
        for &sid in &self.arena.switcher_ids.clone() {
            let (active, prev_opt, children) = if let Some(node) = self.arena.nodes.get_mut(sid)
                && let WidgetPayload::Switcher {
                    signal,
                    children,
                    last_active,
                    prev_idx,
                    transition,
                } = &mut node.payload
            {
                let active = signal.try_get().unwrap_or(0);
                if active != *last_active {
                    *prev_idx = Some(*last_active);
                    *last_active = active;
                    transition.snap_to(0.0);
                    transition.set_target(1.0);
                }
                let moving = transition.step(dt);
                animating |= moving;
                if !moving {
                    *prev_idx = None;
                }
                (active, *prev_idx, children.clone())
            } else {
                continue;
            };

            for (i, &cid) in children.iter().enumerate() {
                let hidden = i != active && Some(i) != prev_opt;
                let is_outgoing = Some(i) == prev_opt && i != active;
                if let Some(node) = self.arena.nodes.get_mut(cid) {
                    let target_pos = if is_outgoing {
                        PositionMode::Absolute
                    } else {
                        PositionMode::Relative
                    };
                    let target_inset = if is_outgoing {
                        Inset::all(Length::Px(0.0))
                    } else {
                        Inset::AUTO
                    };
                    if node.layout_style.hidden != hidden
                        || node.layout_style.position != target_pos
                    {
                        node.layout_style.hidden = hidden;
                        node.layout_style.position = target_pos;
                        node.layout_style.inset = target_inset;
                        let style = node.layout_style;
                        let ln = node.layout_node;
                        self.arena.layout.set_style(ln, style);
                        self.arena.needs_full_layout = true;
                    }
                }
            }
        }

        let now = Instant::now();
        if let Some(blink_at) = self.next_blink_at
            && now >= blink_at
        {
            self.caret_visible = !self.caret_visible;
            self.next_blink_at = Some(now + CARET_BLINK_INTERVAL);
        }

        if let Some(show_at) = self.tooltip_show_at
            && now >= show_at
        {
            self.tooltip_visible = true;
            self.tooltip_show_at = None;
        }

        self.was_animating = animating;
        animating
    }

    fn next_redraw_at(&self) -> Option<Instant> {
        match (self.next_blink_at, self.tooltip_show_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        if let Some(target) = self.captured.or(self.hovered)
            && let Some(node) = self.arena.nodes.get(target)
        {
            match node.payload {
                WidgetPayload::SplitPane {
                    direction: SplitDirection::Horizontal,
                    ..
                } => return CursorStyle::ColResize,
                WidgetPayload::SplitPane {
                    direction: SplitDirection::Vertical,
                    ..
                } => return CursorStyle::RowResize,
                WidgetPayload::TextInput { .. } | WidgetPayload::TextArea { .. } => {
                    return CursorStyle::Default;
                }
                _ => return CursorStyle::Pointer,
            }
        }
        CursorStyle::Default
    }

    fn is_drag_region(&self, x: f32, y: f32) -> bool {
        if self.context_menu.is_some() || self.hit_test(x, y).is_some() {
            return false;
        }
        self.arena
            .root
            .map(|root| is_drag_node(root, &self.arena.nodes, x, y, (0.0, 0.0)))
            .unwrap_or(false)
    }

    fn on_scroll(&mut self, _delta_x: f32, delta_y: f32) -> bool {
        let mut handled = self.clear_tooltip();
        let (cx, cy) = self.last_cursor;
        if let Some(root) = self.arena.root {
            handled |=
                dispatch_scroll_node(root, &mut self.arena.nodes, cx, cy, delta_y, (0.0, 0.0));
        }
        handled
    }

    fn set_a11y_active(&mut self, active: bool) {
        self.a11y_active = active;
        if !active {
            self.cached_semantic_tree = None;
        }
    }

    fn semantic_tree(&self) -> Option<SemanticTree> {
        self.cached_semantic_tree.clone()
    }

    fn on_semantic_action(&mut self, action: A11yAction) -> bool {
        match action {
            A11yAction::Focus(wid) => {
                if self.arena.nodes.contains_key(wid) {
                    let changed = self.arena.focus.set_focus(Some(wid));
                    self.reset_caret_blink();
                    return changed;
                }
                false
            }
            A11yAction::Click(wid) => {
                let Some(node) = self.arena.nodes.get_mut(wid) else {
                    return false;
                };
                match &mut node.payload {
                    WidgetPayload::Button { on_click, .. } => {
                        if let Some(cb) = on_click {
                            cb();
                        }
                        true
                    }
                    WidgetPayload::Switch { signal } => {
                        signal.update(|v| *v = !*v);
                        true
                    }
                    WidgetPayload::Dropdown { is_open, .. } => {
                        *is_open = !*is_open;
                        true
                    }
                    WidgetPayload::TextInput { .. } => {
                        self.arena.focus.set_focus(Some(wid));
                        self.reset_caret_blink();
                        true
                    }
                    _ => false,
                }
            }
        }
    }

    fn on_key(&mut self, key: &Key, text: Option<&str>, modifiers: &Modifiers) -> bool {
        if *key == Key::Escape && self.context_menu.take().is_some() {
            return true;
        }
        if *key == Key::Tab {
            if modifiers.shift {
                self.arena.focus.cycle_prev();
            } else {
                self.arena.focus.cycle_next();
            }
            self.reset_caret_blink();
            return true;
        }

        let Some(focused_id) = self.arena.focus.focused else {
            return false;
        };

        let Some(node) = self.arena.nodes.get_mut(focused_id) else {
            return false;
        };

        let mut handled = false;
        if let WidgetPayload::TextInput {
            signal,
            cursor,
            selection,
            ..
        } = &mut node.payload
        {
            let Some(mut val) = signal.try_get() else {
                return false;
            };
            *cursor = snap_char_boundary(&val, *cursor);
            *selection = normalize_selection(&val, *selection);

            let is_ctrl = modifiers.ctrl || modifiers.super_key;
            let is_shortcut_char = |ch: &str| -> bool {
                matches!(key, Key::Character(c) if c.eq_ignore_ascii_case(ch))
            };

            let delete_sel =
                |val: &mut String, cursor: &mut usize, sel: &mut Option<(usize, usize)>| -> bool {
                    if let Some((start, end)) = sel.take() {
                        let s = snap_char_boundary(val, start.min(end));
                        let e = snap_char_boundary(val, start.max(end));
                        if s < e {
                            val.drain(s..e);
                        }
                        *cursor = s;
                        return true;
                    }
                    false
                };

            if is_ctrl && is_shortcut_char("a") {
                *selection = Some((0, val.len()));
                *cursor = val.len();
                handled = true;
            } else if is_ctrl && is_shortcut_char("c") {
                if let Some((start, end)) = *selection {
                    let s = snap_char_boundary(&val, start.min(end));
                    let e = snap_char_boundary(&val, start.max(end));
                    let _ = Clipboard::set_text(&val[s..e]);
                    handled = true;
                }
            } else if is_ctrl && is_shortcut_char("x") {
                if let Some((start, end)) = *selection {
                    let s = snap_char_boundary(&val, start.min(end));
                    let e = snap_char_boundary(&val, start.max(end));
                    let _ = Clipboard::set_text(&val[s..e]);
                    delete_sel(&mut val, cursor, selection);
                    signal.set(val);
                    handled = true;
                }
            } else if is_ctrl && is_shortcut_char("v") {
                if let Ok(pasted) = Clipboard::get_text() {
                    let clean = pasted.replace('\r', "").replace('\n', " ");
                    delete_sel(&mut val, cursor, selection);
                    val.insert_str(*cursor, &clean);
                    *cursor += clean.len();
                    signal.set(val);
                    handled = true;
                }
            } else {
                match key {
                    Key::Backspace => {
                        if delete_sel(&mut val, cursor, selection) {
                            signal.set(val);
                            handled = true;
                        } else if *cursor > 0 && !val.is_empty() {
                            let prev = val[..*cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            val.drain(prev..*cursor);
                            *cursor = prev;
                            signal.set(val);
                            handled = true;
                        }
                    }
                    Key::Delete => {
                        if delete_sel(&mut val, cursor, selection) {
                            signal.set(val);
                            handled = true;
                        } else if *cursor < val.len() {
                            let next = val[*cursor..]
                                .char_indices()
                                .nth(1)
                                .map(|(i, _)| *cursor + i)
                                .unwrap_or(val.len());
                            val.drain(*cursor..next);
                            signal.set(val);
                            handled = true;
                        }
                    }
                    Key::ArrowLeft => {
                        *selection = None;
                        let next = if is_ctrl {
                            word_prev(&val, *cursor)
                        } else {
                            val[..*cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0)
                        };
                        if next != *cursor {
                            *cursor = next;
                            handled = true;
                        }
                    }
                    Key::ArrowRight => {
                        *selection = None;
                        let next = if is_ctrl {
                            word_next(&val, *cursor)
                        } else {
                            val[*cursor..]
                                .char_indices()
                                .nth(1)
                                .map(|(i, _)| *cursor + i)
                                .unwrap_or(val.len())
                        };
                        if next != *cursor {
                            *cursor = next;
                            handled = true;
                        }
                    }
                    Key::Home => {
                        *selection = None;
                        *cursor = 0;
                        handled = true;
                    }
                    Key::End => {
                        *selection = None;
                        *cursor = val.len();
                        handled = true;
                    }
                    _ => {
                        if !is_ctrl
                            && let Some(ch_str) = text
                            && !ch_str.chars().any(|c| c.is_control())
                        {
                            delete_sel(&mut val, cursor, selection);
                            val.insert_str(*cursor, ch_str);
                            *cursor += ch_str.len();
                            signal.set(val);
                            handled = true;
                        }
                    }
                }
            }
        }

        if !handled
            && let WidgetPayload::TextArea {
                signal,
                cursor,
                selection,
                text_block,
                ..
            } = &mut node.payload
        {
            let Some(mut val) = signal.try_get() else {
                return false;
            };
            *cursor = snap_char_boundary(&val, *cursor);
            *selection = normalize_selection(&val, *selection);

            let is_ctrl = modifiers.ctrl || modifiers.super_key;
            let is_shortcut_char = |ch: &str| -> bool {
                matches!(key, Key::Character(c) if c.eq_ignore_ascii_case(ch))
            };
            let delete_sel =
                |val: &mut String, cursor: &mut usize, sel: &mut Option<(usize, usize)>| -> bool {
                    if let Some((start, end)) = sel.take() {
                        let s = snap_char_boundary(val, start.min(end));
                        let e = snap_char_boundary(val, start.max(end));
                        if s < e {
                            val.drain(s..e);
                        }
                        *cursor = s;
                        return true;
                    }
                    false
                };

            if is_ctrl && is_shortcut_char("a") {
                *selection = Some((0, val.len()));
                *cursor = val.len();
                handled = true;
            } else if is_ctrl && is_shortcut_char("c") {
                if let Some((start, end)) = *selection {
                    let s = snap_char_boundary(&val, start.min(end));
                    let e = snap_char_boundary(&val, start.max(end));
                    let _ = Clipboard::set_text(&val[s..e]);
                    handled = true;
                }
            } else if is_ctrl && is_shortcut_char("x") {
                if let Some((start, end)) = *selection {
                    let s = snap_char_boundary(&val, start.min(end));
                    let e = snap_char_boundary(&val, start.max(end));
                    let _ = Clipboard::set_text(&val[s..e]);
                    delete_sel(&mut val, cursor, selection);
                    signal.set(val);
                    handled = true;
                }
            } else if is_ctrl && is_shortcut_char("v") {
                if let Ok(pasted) = Clipboard::get_text() {
                    delete_sel(&mut val, cursor, selection);
                    val.insert_str(*cursor, &pasted);
                    *cursor += pasted.len();
                    signal.set(val);
                    handled = true;
                }
            } else {
                match key {
                    Key::Enter => {
                        delete_sel(&mut val, cursor, selection);
                        val.insert(*cursor, '\n');
                        *cursor += 1;
                        signal.set(val);
                        handled = true;
                    }
                    Key::Backspace => {
                        if delete_sel(&mut val, cursor, selection) {
                            signal.set(val);
                            handled = true;
                        } else if *cursor > 0 {
                            let prev = val[..*cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            val.drain(prev..*cursor);
                            *cursor = prev;
                            signal.set(val);
                            handled = true;
                        }
                    }
                    Key::Delete => {
                        if delete_sel(&mut val, cursor, selection) {
                            signal.set(val);
                            handled = true;
                        } else if *cursor < val.len() {
                            let next = val[*cursor..]
                                .char_indices()
                                .nth(1)
                                .map(|(i, _)| *cursor + i)
                                .unwrap_or(val.len());
                            val.drain(*cursor..next);
                            signal.set(val);
                            handled = true;
                        }
                    }
                    Key::ArrowLeft => {
                        *selection = None;
                        let next = if is_ctrl {
                            word_prev(&val, *cursor)
                        } else {
                            val[..*cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0)
                        };
                        if next != *cursor {
                            *cursor = next;
                            handled = true;
                        }
                    }
                    Key::ArrowRight => {
                        *selection = None;
                        let next = if is_ctrl {
                            word_next(&val, *cursor)
                        } else {
                            val[*cursor..]
                                .char_indices()
                                .nth(1)
                                .map(|(i, _)| *cursor + i)
                                .unwrap_or(val.len())
                        };
                        if next != *cursor {
                            *cursor = next;
                            handled = true;
                        }
                    }
                    Key::ArrowUp | Key::ArrowDown => {
                        let delta = if *key == Key::ArrowUp { -1 } else { 1 };
                        *selection = None;
                        *cursor =
                            snap_char_boundary(&val, text_block.move_vertical_byte(*cursor, delta));
                        handled = true;
                    }
                    Key::Home => {
                        *selection = None;
                        *cursor = line_range(&val, line_index_at(&val, *cursor)).0;
                        handled = true;
                    }
                    Key::End => {
                        *selection = None;
                        *cursor = line_range(&val, line_index_at(&val, *cursor)).1;
                        handled = true;
                    }
                    _ => {
                        if !is_ctrl
                            && let Some(ch_str) = text
                            && !ch_str.chars().any(|c| c.is_control())
                        {
                            delete_sel(&mut val, cursor, selection);
                            val.insert_str(*cursor, ch_str);
                            *cursor += ch_str.len();
                            signal.set(val);
                            handled = true;
                        }
                    }
                }
            }
        }

        if handled {
            if matches!(node.payload, WidgetPayload::TextArea { .. }) {
                ensure_textarea_caret_visible(node);
            }
            self.reset_caret_blink();
        }
        handled
    }

    fn on_pointer_move(&mut self, x: f32, y: f32) -> bool {
        self.last_cursor = (x, y);
        if let Some(cap) = self.captured
            && let Some(node) = self.arena.nodes.get(cap)
            && let WidgetPayload::SplitPane {
                direction,
                ratio,
                min_ratio,
                max_ratio,
                ..
            } = &node.payload
        {
            let next_r = match direction {
                SplitDirection::Horizontal => (x - node.bounds.x) / node.bounds.width.max(1.0),
                SplitDirection::Vertical => (y - node.bounds.y) / node.bounds.height.max(1.0),
            }
            .clamp(*min_ratio, *max_ratio);
            ratio.set(next_r);
            return true;
        }
        if let Some(cap) = self.captured
            && let Some(node) = self.arena.nodes.get_mut(cap)
            && let WidgetPayload::TextInput {
                signal,
                cursor,
                selection,
                is_selecting,
                char_offsets,
                ..
            } = &mut node.payload
            && *is_selecting
        {
            let val = signal.try_get().unwrap_or_default();
            let rel_x = x - (node.bounds.x + 12.0);
            let anchor = snap_char_boundary(&val, selection.map(|s| s.0).unwrap_or(*cursor));
            let new_cursor = snap_char_boundary(&val, find_char_index_at_x(char_offsets, rel_x));
            *cursor = new_cursor;
            *selection = if anchor != new_cursor {
                Some((anchor, new_cursor))
            } else {
                None
            };
            return true;
        }
        if let Some(cap) = self.captured
            && let Some(node) = self.arena.nodes.get_mut(cap)
            && let WidgetPayload::TextArea {
                signal,
                cursor,
                selection,
                is_selecting,
                text_block,
                scroll_y,
                ..
            } = &mut node.payload
            && *is_selecting
        {
            let val = signal.try_get().unwrap_or_default();
            let rel_x = x - (node.bounds.x + 10.0);
            let rel_y = y - (node.bounds.y + 10.0) + *scroll_y;
            let anchor = snap_char_boundary(&val, selection.map(|s| s.0).unwrap_or(*cursor));
            let idx = text_block.global_byte_at_point(rel_x, rel_y);
            let new_cursor = snap_char_boundary(&val, idx);
            *cursor = new_cursor;
            *selection = if anchor != new_cursor {
                Some((anchor, new_cursor))
            } else {
                None
            };
            return true;
        }
        if let Some(cap) = self.captured {
            return self.update_slider(cap, x);
        }

        let mut redraw = false;

        if let Some(cm) = &mut self.context_menu {
            let next_row = if cm.menu_rect.contains(x, y) {
                let mut found = None;
                let mut cur_y = cm.menu_rect.y + 4.0;
                for (i, item) in cm.labels.iter().enumerate() {
                    let h = if item.is_some() { 28.0 } else { 9.0 };
                    if item.is_some() && y >= cur_y && y < cur_y + h {
                        found = Some(i);
                        break;
                    }
                    cur_y += h;
                }
                found
            } else {
                None
            };
            if cm.hovered_idx != next_row {
                cm.hovered_idx = next_row;
                redraw = true;
            }
            return redraw;
        }

        if !self.open_dropdowns.is_empty() {
            let mut next_dd_row = None;
            for &(dd_id, menu_rect, count) in self.open_dropdowns.iter().rev() {
                if menu_rect.contains(x, y) {
                    let idx = ((y - (menu_rect.y + 4.0)) / 30.0).floor().max(0.0) as usize;
                    next_dd_row = Some((dd_id, idx.min(count.saturating_sub(1))));
                    break;
                }
            }
            if next_dd_row != self.hovered_dropdown_row {
                self.hovered_dropdown_row = next_dd_row;
                redraw = true;
            }
            if next_dd_row.is_some() {
                redraw |= self.clear_tooltip();
                return redraw;
            }
        } else if self.hovered_dropdown_row.take().is_some() {
            redraw = true;
        }

        for &vid in &self.arena.virtual_list_ids {
            let screen_bounds = self
                .arena
                .root
                .and_then(|root| widget_screen_bounds(root, &self.arena.nodes, vid, (0.0, 0.0)));
            if let (Some(screen), Some(node)) = (screen_bounds, self.arena.nodes.get(vid))
                && let WidgetPayload::VirtualizedList { state } = &node.payload
            {
                let mut st = state.borrow_mut();
                let next_row = if screen.contains(x, y) {
                    let stride = st.item_height + st.gap;
                    let idx = ((y - screen.y + st.scroll_y.value) / stride).floor() as isize;
                    if idx >= 0 && (idx as usize) < st.item_count {
                        Some(idx as usize)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if st.hovered_row != next_row {
                    st.hovered_row = next_row;
                    redraw = true;
                }
            }
        }

        let next = self.hit_test(x, y);
        for &tid in &self.arena.traffic_ids {
            if let Some(node) = self.arena.nodes.get_mut(tid)
                && let WidgetPayload::TrafficLights { hovered } = &mut node.payload
            {
                let inside = node.bounds.contains(x, y);
                if *hovered != inside {
                    *hovered = inside;
                    redraw = true;
                }
            }
        }

        if next != self.hovered {
            self.hovered = next;
            redraw = true;
        }

        let next_tt = self
            .arena
            .root
            .and_then(|root| find_tooltip_node(root, &self.arena.nodes, x, y, (0.0, 0.0)));

        let next_tt_id = next_tt.map(|(wid, _)| wid);
        if next_tt_id != self.tooltip_id {
            redraw |= self.clear_tooltip();
            if let Some((wid, anchor)) = next_tt {
                self.tooltip_id = Some(wid);
                self.tooltip_anchor = anchor;
                self.tooltip_show_at = Some(Instant::now() + TOOLTIP_HOVER_DELAY);
            }
        }

        redraw
    }

    fn on_secondary_down(&mut self, x: f32, y: f32) -> bool {
        let mut redraw = self.clear_tooltip();

        for (dd_id, _, _) in self.open_dropdowns.drain(..) {
            if let Some(node) = self.arena.nodes.get_mut(dd_id)
                && let WidgetPayload::Dropdown { is_open, .. } = &mut node.payload
            {
                *is_open = false;
                redraw = true;
            }
        }

        let hit = self
            .arena
            .root
            .and_then(|root| find_context_menu_node(root, &self.arena.nodes, x, y, (0.0, 0.0)));

        if let Some(owner) = hit {
            self.context_menu = Some(ContextMenuState {
                owner,
                anchor: (x, y),
                menu_rect: Rect::default(),
                hovered_idx: None,
                labels: Vec::new(),
            });
            return true;
        }

        if self.context_menu.take().is_some() {
            return true;
        }
        redraw
    }

    fn on_pointer_down(&mut self, x: f32, y: f32) -> (bool, WindowCommand) {
        let mut redraw = self.clear_tooltip();

        if let Some(cm) = self.context_menu.take() {
            if cm.menu_rect.contains(x, y)
                && let Some(idx) = cm.hovered_idx
                && let Some(node) = self.arena.nodes.get_mut(cm.owner)
                && let Some(item) = node.context_menu.get_mut(idx)
                && let Some(cb) = &mut item.on_select
            {
                cb();
            }
            return (true, WindowCommand::None);
        }

        if !self.open_dropdowns.is_empty() {
            let overlays = std::mem::take(&mut self.open_dropdowns);
            for (dd_id, menu_rect, count) in overlays.iter().rev().copied() {
                if menu_rect.contains(x, y) {
                    let row_h = 30.0;
                    let clicked_idx = ((y - (menu_rect.y + 4.0)) / row_h).floor().max(0.0) as usize;
                    let idx = clicked_idx.min(count.saturating_sub(1));
                    if let Some(node) = self.arena.nodes.get_mut(dd_id)
                        && let WidgetPayload::Dropdown {
                            signal, is_open, ..
                        } = &mut node.payload
                    {
                        signal.set(idx);
                        *is_open = false;
                    }
                    self.hovered_dropdown_row = None;
                    return (true, WindowCommand::None);
                }
            }
            for (dd_id, _, _) in overlays {
                if let Some(node) = self.arena.nodes.get_mut(dd_id)
                    && let WidgetPayload::Dropdown { is_open, .. } = &mut node.payload
                {
                    *is_open = false;
                }
            }
            self.hovered_dropdown_row = None;
            redraw = true;
        }

        let Some(hit) = self.hit_test(x, y) else {
            let focus_changed = self.arena.focus.set_focus(None);
            self.reset_caret_blink();
            if self.is_drag_region(x, y) {
                return (redraw, WindowCommand::None);
            }
            return (redraw || focus_changed, WindowCommand::None);
        };

        self.arena.focus.set_focus(Some(hit));
        let mut clicked = false;
        let mut capture_target = false;
        let hit_screen = self
            .arena
            .root
            .and_then(|root| widget_screen_bounds(root, &self.arena.nodes, hit, (0.0, 0.0)))
            .unwrap_or_default();

        if let Some(node) = self.arena.nodes.get_mut(hit) {
            match &mut node.payload {
                WidgetPayload::TrafficLights { .. } => {
                    let rel_x = x - node.bounds.x;
                    let cmd = if rel_x <= 18.0 {
                        WindowCommand::Close
                    } else if rel_x <= 38.0 {
                        WindowCommand::Minimize
                    } else {
                        WindowCommand::ToggleMaximize
                    };
                    return (true, cmd);
                }
                WidgetPayload::Button { on_click, .. } => {
                    if let Some(cb) = on_click {
                        cb();
                    }
                    clicked = true;
                }
                WidgetPayload::Container {
                    on_click: Some(cb), ..
                } => {
                    cb();
                    clicked = true;
                }
                WidgetPayload::Image {
                    on_click: Some(cb), ..
                } => {
                    cb();
                    clicked = true;
                }
                WidgetPayload::Switch { signal } => {
                    signal.update(|v| *v = !*v);
                    clicked = true;
                }
                WidgetPayload::Slider { .. } => {
                    capture_target = true;
                    clicked = true;
                }
                WidgetPayload::SplitPane { .. } => {
                    capture_target = true;
                    clicked = true;
                }
                WidgetPayload::VirtualizedList { state } => {
                    let mut st = state.borrow_mut();
                    let stride = st.item_height + st.gap;
                    let idx = ((y - hit_screen.y + st.scroll_y.value) / stride).floor() as isize;
                    if idx >= 0 && (idx as usize) < st.item_count {
                        let i = idx as usize;
                        if let Some(sel) = st.selected {
                            sel.set(i);
                        }
                        if let Some(cb) = &mut st.on_select {
                            cb(i);
                        }
                        clicked = true;
                    }
                }
                WidgetPayload::Tabs { signal, items, .. } => {
                    let tab_w = node.bounds.width / items.len().max(1) as f32;
                    let idx = ((x - node.bounds.x) / tab_w).floor().max(0.0) as usize;
                    signal.set(idx.min(items.len().saturating_sub(1)));
                    clicked = true;
                }
                WidgetPayload::Dropdown { is_open, .. } => {
                    *is_open = !*is_open;
                    clicked = true;
                }
                WidgetPayload::TextInput {
                    signal,
                    cursor,
                    selection,
                    is_selecting,
                    char_offsets,
                    ..
                } => {
                    let val = signal.try_get().unwrap_or_default();
                    let rel_x = x - (node.bounds.x + 12.0);
                    let char_idx =
                        snap_char_boundary(&val, find_char_index_at_x(char_offsets, rel_x));
                    *cursor = char_idx;
                    *selection = None;
                    *is_selecting = true;
                    capture_target = true;
                    clicked = true;
                }
                WidgetPayload::TextArea {
                    signal,
                    cursor,
                    selection,
                    is_selecting,
                    text_block,
                    scroll_y,
                    ..
                } => {
                    let val = signal.try_get().unwrap_or_default();
                    let rel_x = x - (node.bounds.x + 10.0);
                    let rel_y = y - (node.bounds.y + 10.0) + *scroll_y;
                    let idx = text_block.global_byte_at_point(rel_x, rel_y);
                    *cursor = snap_char_boundary(&val, idx);
                    *selection = None;
                    *is_selecting = true;
                    capture_target = true;
                    clicked = true;
                }
                _ => {}
            }
        }

        self.reset_caret_blink();

        if capture_target {
            self.captured = Some(hit);
            self.update_slider(hit, x);
        }
        (redraw || clicked, WindowCommand::None)
    }

    fn on_pointer_up(&mut self) -> bool {
        let mut was = self.captured.is_some();
        if let Some(cap) = self.captured.take()
            && let Some(node) = self.arena.nodes.get_mut(cap)
            && matches!(
                node.payload,
                WidgetPayload::TextInput { .. } | WidgetPayload::TextArea { .. }
            )
        {
            match &mut node.payload {
                WidgetPayload::TextInput { is_selecting, .. }
                | WidgetPayload::TextArea { is_selecting, .. } => *is_selecting = false,
                _ => {}
            }
            was = true;
        }
        was
    }

    fn on_window_focus(&mut self, focused: bool) -> bool {
        if !focused {
            self.captured = None;
            self.hovered = None;
            self.clear_tooltip();
            self.caret_visible = false;
            self.next_blink_at = None;
            return true;
        }
        self.reset_caret_blink();
        true
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) {
        let frame_start = FrameProfiler::begin_timer();
        for handle in self.arena.pending_texture_releases.drain(..) {
            ctx.gpu_mut().release_texture(handle);
        }
        self.profiler.record_cadence(ctx.frame_dt);

        let mode = self
            .theme_signal
            .and_then(|s| s.try_get())
            .unwrap_or(ThemeMode::Light);
        let theme = MacTheme::from_mode(mode);

        // Stage 1: Drain per-arena reactive dirty notifications
        let dirty_batch: Vec<(WidgetId, DirtyFlags)> =
            std::mem::take(&mut *self.arena.dirty_queue.borrow_mut());
        let mut foreach_dirty: Vec<WidgetId> = Vec::new();
        let mut show_dirty: Vec<WidgetId> = Vec::new();
        for (id, flags) in dirty_batch {
            let Some(node) = self.arena.nodes.get(id) else {
                continue;
            };
            let lnode = node.layout_node;
            let is_foreach = matches!(node.payload, WidgetPayload::ForEach { .. });
            let is_show = matches!(node.payload, WidgetPayload::Show { .. });

            if flags.layout {
                self.arena.layout.mark_dirty(lnode);
                let mut cur = id;
                while let Some(n) = self.arena.nodes.get(cur) {
                    if n.is_relayout_boundary || n.parent.is_none() {
                        break;
                    }
                    if let Some(p) = n.parent {
                        cur = p;
                    } else {
                        break;
                    }
                }
                if Some(cur) == self.arena.root {
                    self.arena.needs_full_layout = true;
                } else if let Some(n) = self.arena.nodes.get_mut(cur)
                    && !n.dirty_layout_queued
                {
                    n.dirty_layout_queued = true;
                    self.arena.dirty_layout_roots.push(cur);
                }
            }

            if is_foreach && (flags.state || flags.layout) {
                foreach_dirty.push(id);
            }
            if is_show && (flags.state || flags.layout) {
                show_dirty.push(id);
            }
            let node = self.arena.nodes.get_mut(id).unwrap();
            match &mut node.payload {
                WidgetPayload::Text {
                    block,
                    pending: Some(cell),
                    ..
                } => {
                    let text_val = cell.borrow().clone();
                    block.set_text(ctx.text_engine(), &text_val);
                }
                WidgetPayload::TextInput {
                    signal,
                    cursor,
                    selection,
                    text_block,
                    char_offsets,
                    last_synced_text,
                    ..
                } => {
                    let text_val = signal.try_get().unwrap_or_default();
                    if *last_synced_text != text_val {
                        *last_synced_text = text_val.clone();
                        *cursor = snap_char_boundary(&text_val, *cursor);
                        *selection = normalize_selection(&text_val, *selection);
                        text_block.set_text(ctx.text_engine(), &text_val);
                        text_block.collect_char_offsets(char_offsets);
                    }
                }
                WidgetPayload::TextArea {
                    signal,
                    cursor,
                    selection,
                    text_block,
                    last_synced_text,
                    ..
                } => {
                    let text_val = signal.try_get().unwrap_or_default();
                    if *last_synced_text != text_val {
                        *last_synced_text = text_val.clone();
                        *cursor = snap_char_boundary(&text_val, *cursor);
                        *selection = normalize_selection(&text_val, *selection);
                        text_block.set_text(ctx.text_engine(), &text_val);
                    }
                }
                _ => {}
            }
        }

        for id in show_dirty {
            let (cond, active, mut when_true, mut when_false) =
                if let Some(node) = self.arena.nodes.get_mut(id) {
                    if let WidgetPayload::Show {
                        condition,
                        active,
                        when_true,
                        when_false,
                    } = &mut node.payload
                    {
                        (*condition, *active, when_true.take(), when_false.take())
                    } else {
                        continue;
                    }
                } else {
                    continue;
                };
            let next = cond.try_get().unwrap_or(false);
            if next != active {
                let old_children: Vec<WidgetId> = self
                    .arena
                    .nodes
                    .get(id)
                    .map(|n| n.children.clone())
                    .unwrap_or_default();
                for child in old_children {
                    self.arena.unmount_subtree(child);
                }
                let new_child = if next {
                    when_true
                        .as_mut()
                        .map(|f| self.arena.mount_node(f(), ctx.text_engine()))
                } else {
                    when_false
                        .as_mut()
                        .map(|f| self.arena.mount_node(f(), ctx.text_engine()))
                };
                let children: Vec<WidgetId> = new_child.into_iter().collect();
                for &child in &children {
                    if let Some(node) = self.arena.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }
                let child_lnodes: Vec<LayoutNode> = children
                    .iter()
                    .filter_map(|cid| self.arena.nodes.get(*cid).map(|n| n.layout_node))
                    .collect();
                if let Some(node) = self.arena.nodes.get_mut(id) {
                    node.children = children;
                    let lnode = node.layout_node;
                    if let WidgetPayload::Show {
                        active,
                        when_true: wt,
                        when_false: wf,
                        ..
                    } = &mut node.payload
                    {
                        *active = next;
                        *wt = when_true;
                        *wf = when_false;
                    }
                    self.arena.layout.set_children(lnode, &child_lnodes);
                    self.arena.layout.mark_dirty(lnode);
                }
                self.arena.needs_full_layout = true;
            } else if let Some(node) = self.arena.nodes.get_mut(id)
                && let WidgetPayload::Show {
                    when_true: wt,
                    when_false: wf,
                    ..
                } = &mut node.payload
            {
                *wt = when_true;
                *wf = when_false;
            }
        }

        for id in foreach_dirty {
            let reconcile_fn = if let Some(node) = self.arena.nodes.get_mut(id) {
                if let WidgetPayload::ForEach { reconcile } = &mut node.payload {
                    Some(std::mem::replace(reconcile, Box::new(|_, _, _| {})))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(mut reconcile_fn) = reconcile_fn {
                reconcile_fn(&mut self.arena, ctx.text_engine(), id);
                if let Some(node) = self.arena.nodes.get_mut(id)
                    && let WidgetPayload::ForEach { reconcile } = &mut node.payload
                {
                    *reconcile = reconcile_fn;
                }
                self.arena.needs_full_layout = true;
            }
        }

        let mut split_syncs = Vec::new();
        for &sp_id in &self.arena.split_ids {
            if let Some(node) = self.arena.nodes.get(sp_id)
                && let WidgetPayload::SplitPane {
                    direction,
                    ratio,
                    min_ratio,
                    max_ratio,
                    first_id,
                    ..
                } = &node.payload
            {
                let r = ratio.try_get().unwrap_or(0.3).clamp(*min_ratio, *max_ratio);
                split_syncs.push((*first_id, *direction, r));
            }
        }
        for (first_id, dir, r) in split_syncs {
            if let Some(first_node) = self.arena.nodes.get_mut(first_id) {
                let target_len = Length::Percent(r);
                let changed = match dir {
                    SplitDirection::Horizontal => first_node.layout_style.width != target_len,
                    SplitDirection::Vertical => first_node.layout_style.height != target_len,
                };
                if changed {
                    match dir {
                        SplitDirection::Horizontal => first_node.layout_style.width = target_len,
                        SplitDirection::Vertical => first_node.layout_style.height = target_len,
                    }
                    self.arena
                        .layout
                        .set_style(first_node.layout_node, first_node.layout_style);
                    self.arena.needs_full_layout = true;
                }
            }
        }

        if (ctx.width, ctx.height) != self.last_size {
            self.last_size = (ctx.width, ctx.height);
            if let Some(viewport) = self.viewport_signal {
                viewport.set((ctx.width, ctx.height));
            }
            self.arena.needs_full_layout = true;
        }

        // Stage 2: Incremental Taffy Layout + ScrollView content_height
        let layout_start = FrameProfiler::begin_timer();
        let mut did_layout = false;
        if (self.arena.needs_full_layout || self.arena.dirty_layout_roots.len() > 32)
            && let Some(root) = self.arena.root
        {
            let root_lnode = self.arena.nodes[root].layout_node;
            let mut rstyle = self.arena.nodes[root].layout_style;
            rstyle.width = Length::Px(ctx.width);
            rstyle.height = Length::Px(ctx.height);
            self.arena.layout.set_style(root_lnode, rstyle);

            let nodes = &mut self.arena.nodes;
            let lmap = &self.arena.layout_to_widget;
            let text_engine = ctx.text_engine();

            self.arena.layout.compute_with_measure(
                root_lnode,
                self.last_size.0,
                self.last_size.1,
                |lnode, _, _, avail_w, _| {
                    let Some(&wid) = lmap.get(&lnode) else {
                        return (0.0, 0.0);
                    };
                    let Some(node) = nodes.get_mut(wid) else {
                        return (0.0, 0.0);
                    };
                    match &mut node.payload {
                        WidgetPayload::Text { block, .. } => block.measure(text_engine, avail_w),
                        WidgetPayload::TextArea { text_block, .. } => {
                            text_block.measure(text_engine, avail_w.map(|w| (w - 20.0).max(1.0)))
                        }
                        WidgetPayload::Button {
                            label,
                            label_empty,
                            icon_state,
                            icon_size,
                            ..
                        } => {
                            let (w, _) = label.measure(text_engine, None);
                            let has_icon = icon_state.borrow().handle.is_some()
                                || icon_state.borrow().image.is_some();
                            if has_icon && *label_empty {
                                (30.0, 30.0)
                            } else if has_icon {
                                (w + 28.0 + *icon_size, 30.0)
                            } else {
                                (w + 28.0, 30.0)
                            }
                        }
                        _ => (0.0, 0.0),
                    }
                },
            );

            let rects = self.arena.layout.collect_logical_rects(root_lnode);
            for (&lnode, &rect) in rects {
                if let Some(&wid) = self.arena.layout_to_widget.get(&lnode)
                    && let Some(node) = self.arena.nodes.get_mut(wid)
                {
                    node.bounds = rect;
                }
            }

            for &sid in &self.arena.scroll_ids {
                let cid = match self.arena.nodes.get(sid).map(|n| &n.payload) {
                    Some(WidgetPayload::ScrollView { content_id, .. }) => *content_id,
                    _ => continue,
                };
                let ch = self
                    .arena
                    .nodes
                    .get(cid)
                    .map(|n| n.bounds.height)
                    .unwrap_or(0.0);
                if let Some(snode) = self.arena.nodes.get_mut(sid)
                    && let WidgetPayload::ScrollView { content_height, .. } = &mut snode.payload
                {
                    *content_height = ch;
                }
            }

            self.arena.needs_full_layout = false;
            for &id in &self.arena.dirty_layout_roots {
                if let Some(n) = self.arena.nodes.get_mut(id) {
                    n.dirty_layout_queued = false;
                }
            }
            self.arena.dirty_layout_roots.clear();
            did_layout = true;
        } else if !self.arena.dirty_layout_roots.is_empty() {
            let roots: Vec<WidgetId> = std::mem::take(&mut self.arena.dirty_layout_roots);
            for boundary_id in roots {
                if let Some(n) = self.arena.nodes.get_mut(boundary_id) {
                    n.dirty_layout_queued = false;
                }
                let Some(boundary_node) = self.arena.nodes.get(boundary_id) else {
                    continue;
                };
                let boundary_lnode = boundary_node.layout_node;
                let bw = boundary_node.bounds.width.max(1.0);
                let bh = boundary_node.bounds.height.max(1.0);
                let origin_x = boundary_node.bounds.x;
                let origin_y = boundary_node.bounds.y;

                let nodes = &mut self.arena.nodes;
                let lmap = &self.arena.layout_to_widget;
                let text_engine = ctx.text_engine();

                self.arena.layout.compute_subtree_with_measure(
                    boundary_lnode,
                    bw,
                    bh,
                    |lnode, _, _, avail_w, _| {
                        let Some(&wid) = lmap.get(&lnode) else {
                            return (0.0, 0.0);
                        };
                        let Some(node) = nodes.get_mut(wid) else {
                            return (0.0, 0.0);
                        };
                        match &mut node.payload {
                            WidgetPayload::Text { block, .. } => {
                                block.measure(text_engine, avail_w)
                            }
                            WidgetPayload::TextArea { text_block, .. } => text_block
                                .measure(text_engine, avail_w.map(|w| (w - 20.0).max(1.0))),
                            WidgetPayload::Button {
                                label,
                                label_empty,
                                icon_state,
                                icon_size,
                                ..
                            } => {
                                let (w, _) = label.measure(text_engine, None);
                                let has_icon = icon_state.borrow().handle.is_some()
                                    || icon_state.borrow().image.is_some();
                                if has_icon && *label_empty {
                                    (30.0, 30.0)
                                } else if has_icon {
                                    (w + 28.0 + *icon_size, 30.0)
                                } else {
                                    (w + 28.0, 30.0)
                                }
                            }
                            _ => (0.0, 0.0),
                        }
                    },
                );

                let slice = self.arena.layout.collect_logical_rects_into(
                    boundary_lnode,
                    origin_x,
                    origin_y,
                );
                self.arena.rect_buf.clear();
                self.arena.rect_buf.extend_from_slice(slice);
                for i in 0..self.arena.rect_buf.len() {
                    let (lnode, rect) = self.arena.rect_buf[i];
                    if let Some(&wid) = self.arena.layout_to_widget.get(&lnode)
                        && let Some(node) = self.arena.nodes.get_mut(wid)
                    {
                        node.bounds = rect;
                    }
                }
            }
            did_layout = true;
        }
        let layout_ms = layout_start.elapsed().as_secs_f32() * 1000.0;

        if let Some(cm) = &mut self.context_menu
            && cm.labels.is_empty()
            && let Some(owner_node) = self.arena.nodes.get(cm.owner)
        {
            let te = ctx.text_engine();
            for item in &owner_node.context_menu {
                if let Some(lbl) = &item.label {
                    let normal = TextBlock::new(
                        te,
                        lbl,
                        TextStyle {
                            font_size: 13.0,
                            line_height: 18.0,
                            color: theme.text_primary,
                            weight: FontWeight::Medium,
                        },
                    );
                    let selected = TextBlock::new(
                        te,
                        lbl,
                        TextStyle {
                            font_size: 13.0,
                            line_height: 18.0,
                            color: theme.text_on_accent,
                            weight: FontWeight::Medium,
                        },
                    );
                    cm.labels.push(Some((normal, selected)));
                } else {
                    cm.labels.push(None);
                }
            }
        }

        if self.tooltip_visible
            && self.tooltip_cached_id != self.tooltip_id
            && let Some(tid) = self.tooltip_id
            && let Some(tt_text) = self.arena.nodes.get(tid).and_then(|n| n.tooltip.as_deref())
        {
            let te = ctx.text_engine();
            let style = TextStyle {
                font_size: 11.5,
                line_height: 15.0,
                color: theme.text_primary,
                weight: FontWeight::Medium,
            };
            self.tooltip_block = Some(TextBlock::new(te, tt_text, style));
            self.tooltip_cached_id = Some(tid);
        }

        // Stage 3: Hierarchical Paint + Overlays + Lazy SemanticTree
        let paint_start = FrameProfiler::begin_timer();
        let mut overlay_records = Vec::new();
        self.open_dropdowns.clear();

        if let Some(draw) = &mut self.path_draw_fn {
            draw(ctx);
        }

        if let Some(root) = self.arena.root {
            let scale = ctx.scale;
            let time_secs = self.start_instant.elapsed().as_secs_f32();
            let mut a11y_buf = if self.a11y_active {
                Some(Vec::new())
            } else {
                None
            };
            let mut selection_scratch = Vec::new();
            let mut pctx = PaintCtx {
                ctx,
                theme: &theme,
                scale,
                time_secs,
                caret_visible: self.caret_visible,
                hovered: self.hovered,
                focused: self.arena.focus.focused,
                overlays: &mut overlay_records,
                a11y_records: a11y_buf.as_mut(),
                selection_scratch: &mut selection_scratch,
                decoded_tex_cache: &mut self.arena.decoded_tex_cache,
            };

            paint_node(root, &self.arena.nodes, &mut pctx, (0.0, 0.0));

            if let Some(records) = a11y_buf {
                self.cached_semantic_tree = Some(SemanticTree {
                    root,
                    focus: self.arena.focus.focused,
                    nodes: records,
                });
            }

            // 3A. Open Dropdown Overlays
            for ov in overlay_records {
                let row_h = 30.0;
                let menu_w = ov.trigger_bounds.width.max(160.0).min(ctx.width - 16.0);
                let menu_h = ov.item_count as f32 * row_h + 8.0;
                let menu_x = ov
                    .trigger_bounds
                    .x
                    .clamp(8.0, (ctx.width - menu_w - 8.0).max(8.0));

                let space_below =
                    ctx.height - (ov.trigger_bounds.y + ov.trigger_bounds.height + 4.0);
                let menu_y = if space_below >= menu_h + 8.0 {
                    ov.trigger_bounds.y + ov.trigger_bounds.height + 4.0
                } else {
                    (ov.trigger_bounds.y - menu_h - 4.0).max(8.0)
                };
                let menu_rect = Rect::new(menu_x, menu_y, menu_w, menu_h);
                self.open_dropdowns
                    .push((ov.source_id, menu_rect, ov.item_count));

                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(menu_rect, scale),
                    radii: CornerRadii::all(10.0).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: theme.shadow_elevated,
                    shadow_offset: [0.0, 8.0 * scale],
                    shadow_blur: 20.0 * scale,
                    ..Default::default()
                });
                ctx.push_glass(GlassQuad {
                    rect: snap_rect_to_pixels(menu_rect, scale),
                    radii: CornerRadii::all(10.0).scale(scale),
                    tint: theme.popover_glass_tint,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                    frost: 0.42,
                    specular: 0.85,
                    ..Default::default()
                });

                if let Some(dd_node) = self.arena.nodes.get(ov.source_id)
                    && let WidgetPayload::Dropdown {
                        labels,
                        selected_labels,
                        ..
                    } = &dd_node.payload
                {
                    for i in 0..ov.item_count {
                        let ry = menu_y + 4.0 + i as f32 * row_h;
                        let row_rect = Rect::new(menu_x + 4.0, ry, menu_w - 8.0, row_h);
                        let is_sel = i == ov.selected;
                        let is_hov = self.hovered_dropdown_row == Some((ov.source_id, i));
                        if is_sel || is_hov {
                            ctx.push_sdf(SdfQuad {
                                rect: snap_rect_to_pixels(row_rect, scale),
                                radii: CornerRadii::all(6.0).scale(scale),
                                fill: if is_sel {
                                    theme.accent
                                } else {
                                    theme.control_hover
                                },
                                ..Default::default()
                            });
                        }
                        let block = if is_sel {
                            &selected_labels[i]
                        } else {
                            &labels[i]
                        };
                        ctx.push_text_colored(
                            block,
                            row_rect.x + 10.0,
                            row_rect.y + 6.0,
                            if is_sel {
                                theme.text_on_accent
                            } else {
                                theme.text_primary
                            },
                        );
                    }
                }
            }

            // 3B. Active Right-Click ContextMenu Overlay
            if let Some(cm) = &mut self.context_menu
                && !cm.labels.is_empty()
            {
                let menu_w = 196.0_f32.min(ctx.width - 16.0);
                let menu_h = cm
                    .labels
                    .iter()
                    .map(|l| if l.is_some() { 28.0 } else { 9.0 })
                    .sum::<f32>()
                    + 8.0;

                let menu_x = if cm.anchor.0 + menu_w <= ctx.width - 8.0 {
                    cm.anchor.0
                } else {
                    (cm.anchor.0 - menu_w).max(8.0)
                };
                let menu_y = if cm.anchor.1 + menu_h <= ctx.height - 8.0 {
                    cm.anchor.1
                } else {
                    (cm.anchor.1 - menu_h).max(8.0)
                };

                let menu_rect = Rect::new(menu_x, menu_y, menu_w, menu_h);
                cm.menu_rect = menu_rect;

                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(menu_rect, scale),
                    radii: CornerRadii::all(10.0).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: theme.shadow_elevated,
                    shadow_offset: [0.0, 10.0 * scale],
                    shadow_blur: 22.0 * scale,
                    ..Default::default()
                });
                ctx.push_glass(GlassQuad {
                    rect: snap_rect_to_pixels(menu_rect, scale),
                    radii: CornerRadii::all(10.0).scale(scale),
                    tint: theme.popover_glass_tint,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                    frost: 0.45,
                    specular: 0.85,
                    ..Default::default()
                });

                let mut cur_y = menu_y + 4.0;
                for (i, item_opt) in cm.labels.iter().enumerate() {
                    if let Some((normal, selected)) = item_opt {
                        let row_rect = Rect::new(menu_x + 4.0, cur_y, menu_w - 8.0, 28.0);
                        let is_hov = cm.hovered_idx == Some(i);
                        if is_hov {
                            ctx.push_sdf(SdfQuad {
                                rect: snap_rect_to_pixels(row_rect, scale),
                                radii: CornerRadii::all(6.0).scale(scale),
                                fill: theme.accent,
                                ..Default::default()
                            });
                        }
                        let blk = if is_hov { selected } else { normal };
                        ctx.push_text_colored(
                            blk,
                            row_rect.x + 10.0,
                            row_rect.y + 5.0,
                            if is_hov {
                                theme.text_on_accent
                            } else {
                                theme.text_primary
                            },
                        );
                        cur_y += 28.0;
                    } else {
                        ctx.push_sdf(SdfQuad {
                            rect: Rect::new(
                                ((menu_x + 12.0) * scale).round(),
                                ((cur_y + 4.0) * scale).round(),
                                ((menu_w - 24.0) * scale).round(),
                                1.0,
                            ),
                            fill: theme.hairline,
                            ..Default::default()
                        });
                        cur_y += 9.0;
                    }
                }
            }

            // 3C. Hover-Delayed Tooltip Pill
            if self.tooltip_visible
                && let Some(tb) = &mut self.tooltip_block
            {
                let (tw, th) = tb.measure(ctx.text_engine(), Some(280.0));
                let box_w = tw + 18.0;
                let box_h = th + 10.0;
                let tx = (self.tooltip_anchor.x + (self.tooltip_anchor.width - box_w) * 0.5)
                    .clamp(8.0, (ctx.width - box_w - 8.0).max(8.0));
                let below_y = self.tooltip_anchor.y + self.tooltip_anchor.height + 6.0;
                let ty = if below_y + box_h <= ctx.height - 8.0 {
                    below_y
                } else {
                    (self.tooltip_anchor.y - box_h - 6.0).max(8.0)
                };

                let tt_rect = Rect::new(tx, ty, box_w, box_h);
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(tt_rect, scale),
                    radii: CornerRadii::all(6.0).scale(scale),
                    fill: theme.panel_bg,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                    shadow_color: theme.shadow_soft,
                    shadow_offset: [0.0, 4.0 * scale],
                    shadow_blur: 12.0 * scale,
                    ..Default::default()
                });
                ctx.push_text_colored(tb, tt_rect.x + 9.0, tt_rect.y + 5.0, theme.text_primary);
            }
        }

        if self.show_fps {
            let m = self.profiler.metrics;
            let should_refresh = self.fps_block.is_none()
                || self.last_fps_update.elapsed() >= Duration::from_millis(150);
            if should_refresh {
                self.last_fps_update = Instant::now();
                // Only show presented FPS when actively animating (springs /
                // shaders) so event-driven interactions don't display CPU-only
                // `1000 / frame_ms` spikes (4000+ FPS) or typing-rate numbers.
                let label = if self.was_animating && m.effective_fps > 10.0 {
                    format!(
                        "{:.0} FPS  •  {:.2} ms (L:{:.2} P:{:.2})",
                        m.effective_fps, m.frame_ms, m.layout_ms, m.paint_ms
                    )
                } else {
                    format!(
                        "Idle  •  {:.2} ms (L:{:.2} P:{:.2})",
                        m.frame_ms, m.layout_ms, m.paint_ms
                    )
                };
                let te = ctx.text_engine();
                if let Some(blk) = &mut self.fps_block {
                    blk.set_text(te, &label);
                } else {
                    self.fps_block = Some(TextBlock::new(
                        te,
                        label,
                        TextStyle {
                            font_size: 11.0,
                            line_height: 14.0,
                            color: theme.text_primary,
                            weight: FontWeight::Medium,
                        },
                    ));
                }
            }

            if let Some(blk) = &mut self.fps_block {
                let (tw, _) = blk.measure(ctx.text_engine(), None);
                let pill_w = tw + 16.0;
                let pill_h = 22.0;
                let pill_rect = Rect::new(
                    (ctx.width - pill_w - 14.0).max(8.0),
                    (ctx.height - pill_h - 10.0).max(8.0),
                    pill_w,
                    pill_h,
                );
                let scale = ctx.scale;
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(pill_rect, scale),
                    radii: CornerRadii::all(6.0).scale(scale),
                    fill: theme.panel_bg,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                    ..Default::default()
                });
                ctx.push_text_colored(
                    blk,
                    pill_rect.x + 8.0,
                    pill_rect.y + 4.0,
                    theme.text_primary,
                );
            }
        }

        let paint_ms = paint_start.elapsed().as_secs_f32() * 1000.0;
        let frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.profiler
            .record_frame(frame_ms, layout_ms, paint_ms, did_layout);
    }
}

