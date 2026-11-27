struct CustomSurfaceState {
    pass: Box<dyn CustomGpuPass>,
    handle: Option<TextureHandle>,
    view: Option<wgpu::TextureView>,
    size: (u32, u32),
}

struct ImageState {
    source_key: Option<usize>,
    image: Option<Arc<DecodedImage>>,
    handle: Option<TextureHandle>,
    width: u32,
    height: u32,
}

struct IconState {
    image: Option<DecodedImage>,
    handle: Option<TextureHandle>,
}

struct VirtualRowSlot {
    cached_index: Option<usize>,
    title: TextBlock,
    subtitle: TextBlock,
    badge: TextBlock,
    badge_width: f32,
}

struct VirtualListState {
    item_count: usize,
    item_height: f32,
    gap: f32,
    flat: bool,
    columns_mode: bool,
    accent: Option<Color>,
    scroll_y: Spring,
    hovered_row: Option<usize>,
    selected: Option<Signal<usize>>,
    on_select: Option<Box<dyn FnMut(usize)>>,
    row_builder: Box<dyn FnMut(usize) -> VirtualRow>,
    row_art: Option<Box<dyn FnMut(usize) -> Arc<DecodedImage>>>,
    art_cache: HashMap<usize, (TextureHandle, u32, u32)>,
    pool: Vec<VirtualRowSlot>,
}

enum WidgetPayload {
    Container {
        material: Rc<Cell<Material>>,
        radius: Rc<Cell<f32>>,
        is_drag_region: bool,
        hover_fill: Option<Color>,
        on_click: Option<Box<dyn FnMut()>>,
    },
    Text {
        block: TextBlock,
        pending: Option<Rc<RefCell<String>>>,
        tone: TextTone,
    },
    Button {
        label: TextBlock,
        label_empty: bool,
        accent: bool,
        variant: ButtonVariant,
        icon_state: RefCell<IconState>,
        icon_size: f32,
        pill: bool,
        on_click: Option<Box<dyn FnMut()>>,
    },
    Image {
        state: RefCell<ImageState>,
        radius: f32,
        tint: Color,
        shadow: Option<Shadow>,
        fit: ImageFit,
        on_click: Option<Box<dyn FnMut()>>,
    },
    Icon {
        state: RefCell<IconState>,
        tone: TextTone,
        color: Color,
        size: f32,
    },
    Switch {
        signal: Signal<bool>,
    },
    Slider {
        signal: Signal<f32>,
        min: f32,
        max: f32,
    },
    ProgressBar {
        signal: Signal<f32>,
        color: Option<Color>,
    },
    TextInput {
        signal: Signal<String>,
        material: Material,
        cursor: usize,
        selection: Option<(usize, usize)>,
        is_selecting: bool,
        text_block: TextBlock,
        placeholder_block: TextBlock,
        char_offsets: Vec<(usize, f32)>,
        last_synced_text: String,
    },
    TextArea {
        signal: Signal<String>,
        material: Material,
        cursor: usize,
        selection: Option<(usize, usize)>,
        is_selecting: bool,
        text_block: TextBlock,
        placeholder_block: TextBlock,
        last_synced_text: String,
        scroll_y: f32,
    },
    ScrollView {
        scroll_y: Spring,
        content_height: f32,
        content_id: WidgetId,
    },
    SplitPane {
        direction: SplitDirection,
        ratio: Signal<f32>,
        min_ratio: f32,
        max_ratio: f32,
        first_id: WidgetId,
        divider_id: WidgetId,
        divider_color: Option<Color>,
    },
    VirtualizedList {
        state: RefCell<VirtualListState>,
    },
    Switcher {
        signal: Signal<usize>,
        children: Vec<WidgetId>,
        last_active: usize,
        prev_idx: Option<usize>,
        transition: Spring,
    },
    Show {
        condition: Signal<bool>,
        active: bool,
        when_true: Option<Box<dyn FnMut() -> Element>>,
        when_false: Option<Box<dyn FnMut() -> Element>>,
    },
    ForEach {
        reconcile: ForEachReconcile,
    },
    Tabs {
        signal: Signal<usize>,
        items: Vec<String>,
        labels: Vec<TextBlock>,
        label_widths: Vec<f32>,
        material: Material,
        pill_spring: Spring,
    },
    Dropdown {
        signal: Signal<usize>,
        items: Vec<String>,
        labels: Vec<TextBlock>,
        selected_labels: Vec<TextBlock>,
        material: Material,
        is_open: bool,
        chevron: TextBlock,
    },
    CustomGpuSurface {
        radius: Rc<Cell<f32>>,
        animate_signal: Option<Signal<bool>>,
        needs_redraw: Rc<Cell<bool>>,
        resolution_scale: f32,
        state: RefCell<CustomSurfaceState>,
    },
    TrafficLights {
        hovered: bool,
    },
    Divider {
        color: Color,
    },
    Spacer,
}

struct WidgetNode {
    children: Vec<WidgetId>,
    layout_node: LayoutNode,
    layout_style: LayoutStyle,
    bounds: Rect,
    owned_effects: Vec<EffectId>,
    tooltip: Option<String>,
    context_menu: Vec<MenuItem>,
    payload: WidgetPayload,
    parent: Option<WidgetId>,
    is_relayout_boundary: bool,
    dirty_layout_queued: bool,
}

/// Owns the live widget tree produced by mounting an [`Element`].
///
/// The arena holds the node slot map, its parallel taffy layout tree, focus
/// state, reactive effects created by widgets, cached image textures, and the
/// queue of pending dirty notifications. Nodes are addressed by [`WidgetId`]
/// and kept alive for as long as they are mounted; effects are disposed and
/// textures queued for release when nodes are removed.
///
/// Most applications use [`WidgetWindow`] instead of interacting with the
/// arena directly; the tests and benchmarks use it to mount, lay out, and
/// inspect a tree without opening a window.
///
/// # Examples
///
/// ```
/// use cadmium::text::TextEngine;
/// use cadmium::widgets::{Container, IntoElement, Text, WidgetArena};
///
/// let mut arena = WidgetArena::new();
/// let mut text = TextEngine::new();
/// let root = arena.mount(
///     Container::new().child(Text::new("Hello")).into_element(),
///     &mut text,
/// );
/// assert_eq!(arena.node_count(), 2);
/// ```
pub struct WidgetArena {
    nodes: SlotMap<WidgetId, WidgetNode>,
    layout: LayoutEngine,
    layout_to_widget: HashMap<LayoutNode, WidgetId>,
    root: Option<WidgetId>,
    focus: FocusManager,
    dirty_queue: DirtyQueue,
    scroll_ids: Vec<WidgetId>,
    tab_ids: Vec<WidgetId>,
    traffic_ids: Vec<WidgetId>,
    custom_surface_ids: Vec<WidgetId>,
    split_ids: Vec<WidgetId>,
    virtual_list_ids: Vec<WidgetId>,
    switcher_ids: Vec<WidgetId>,
    pending_texture_releases: Vec<TextureHandle>,
    dirty_layout_roots: Vec<WidgetId>,
    rect_buf: Vec<(LayoutNode, Rect)>,
    decoded_tex_cache: HashMap<usize, (TextureHandle, usize)>,
    svg_tex_cache: HashMap<(u64, u32, u32), (TextureHandle, usize)>,
    needs_full_layout: bool,
}

impl WidgetArena {
    /// Creates an empty arena with no root, no nodes, and a full layout pending.
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            layout: LayoutEngine::new(),
            layout_to_widget: HashMap::new(),
            root: None,
            focus: FocusManager::new(),
            dirty_queue: Rc::new(RefCell::new(Vec::new())),
            scroll_ids: Vec::new(),
            tab_ids: Vec::new(),
            traffic_ids: Vec::new(),
            custom_surface_ids: Vec::new(),
            split_ids: Vec::new(),
            virtual_list_ids: Vec::new(),
            switcher_ids: Vec::new(),
            pending_texture_releases: Vec::new(),
            dirty_layout_roots: Vec::new(),
            rect_buf: Vec::new(),
            decoded_tex_cache: HashMap::new(),
            svg_tex_cache: HashMap::new(),
            needs_full_layout: true,
        }
    }

    /// Replaces the current tree with `element` and returns the new root id.
    ///
    /// The previous tree is cleared first: its effects are disposed, its
    /// textures are queued for release, and focus is reset. Nodes are created
    /// recursively in depth-first order, and the whole tree is marked for a
    /// full layout pass before the next measurement.
    ///
    /// The returned [`WidgetId`] identifies the root node in this arena. It is
    /// only valid until the next `mount`, `clear`, or unmount of that subtree.
    pub fn mount(&mut self, element: Element, text: &mut TextEngine) -> WidgetId {
        self.clear();
        let root_id = self.mount_node(element, text);
        self.root = Some(root_id);
        self.needs_full_layout = true;
        root_id
    }

    /// Removes every mounted node and releases the resources they owned.
    ///
    /// Reactive effects owned by widgets are disposed, image/icon/custom-pass
    /// textures are queued for GPU release, the layout tree and focus manager
    /// are reset, and the root is cleared. The arena remains usable for a new
    /// [`WidgetArena::mount`].
    pub fn clear(&mut self) {
        let mut released: std::collections::HashSet<TextureHandle> =
            std::collections::HashSet::new();
        for (_, (handle, _)) in self.decoded_tex_cache.drain() {
            if released.insert(handle) {
                self.pending_texture_releases.push(handle);
            }
        }
        for (_, (handle, _)) in self.svg_tex_cache.drain() {
            if released.insert(handle) {
                self.pending_texture_releases.push(handle);
            }
        }

        for (_, node) in self.nodes.drain() {
            for eff in node.owned_effects {
                dispose_effect(eff);
            }
            match node.payload {
                WidgetPayload::CustomGpuSurface { state, .. } => {
                    if let Some(handle) = state.into_inner().handle
                        && released.insert(handle)
                    {
                        self.pending_texture_releases.push(handle);
                    }
                }
                WidgetPayload::Image { state, .. } => {
                    let st = state.into_inner();
                    if let Some(handle) = st.handle {
                        let should_release = if let Some(key) = st.source_key {
                            if let Some((_, refs)) = self.decoded_tex_cache.get_mut(&key) {
                                *refs = refs.saturating_sub(1);
                                if *refs == 0 {
                                    self.decoded_tex_cache.remove(&key);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                true
                            }
                        } else {
                            true
                        };
                        if should_release && released.insert(handle) {
                            self.pending_texture_releases.push(handle);
                        }
                    }
                }
                WidgetPayload::Icon { state, .. } => {
                    if let Some(handle) = state.into_inner().handle
                        && released.insert(handle)
                    {
                        self.pending_texture_releases.push(handle);
                    }
                }
                WidgetPayload::Button { icon_state, .. } => {
                    if let Some(handle) = icon_state.into_inner().handle
                        && released.insert(handle)
                    {
                        self.pending_texture_releases.push(handle);
                    }
                }
                WidgetPayload::VirtualizedList { state } => {
                    for (_, (handle, _, _)) in state.into_inner().art_cache {
                        if released.insert(handle) {
                            self.pending_texture_releases.push(handle);
                        }
                    }
                }
                _ => {}
            }
            self.layout.remove(node.layout_node);
        }
        self.layout_to_widget.clear();
        self.dirty_queue.borrow_mut().clear();
        self.scroll_ids.clear();
        self.tab_ids.clear();
        self.traffic_ids.clear();
        self.custom_surface_ids.clear();
        self.split_ids.clear();
        self.virtual_list_ids.clear();
        self.switcher_ids.clear();
        for &id in &self.dirty_layout_roots {
            if let Some(n) = self.nodes.get_mut(id) {
                n.dirty_layout_queued = false;
            }
        }
        self.dirty_layout_roots.clear();
        self.focus = FocusManager::new();
        self.root = None;
    }

    /// Returns the number of currently mounted widget nodes.
    ///
    /// This includes internal nodes created by widgets, such as the content and
    /// viewport nodes of a [`ScrollView`].
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Removes the subtree rooted at `id`, freeing effects and GPU handles.
    ///
    /// If `id` is not mounted, this does nothing. The traversal is iterative
    /// and post-order (leaves before parents) so deep trees cannot overflow
    /// the stack. Removed nodes also no longer appear in the arena's scroll,
    /// tab, custom-surface, split, virtual-list, or switcher registries.
    ///
    /// In normal use, structural widgets such as [`Show`] and [`ForEach`]
    /// call this themselves; calling it on a child of a mounted container
    /// leaves that container's child list pointing at a stale id.
    pub fn unmount_subtree(&mut self, id: WidgetId) {
        // Phase 1: collect all nodes in the subtree, leaves first (post-order).
        let mut order: Vec<WidgetId> = Vec::new();
        let mut stack: Vec<WidgetId> = vec![id];
        while let Some(wid) = stack.pop() {
            let Some(node) = self.nodes.get(wid) else {
                continue;
            };
            // Push children so they appear before the parent in `order`.
            for &child in &node.children {
                stack.push(child);
            }
            order.push(wid);
        }
        // `order` is currently root-first; reverse for post-order (leaves first).
        order.reverse();

        let mut released: std::collections::HashSet<TextureHandle> =
            std::collections::HashSet::new();

        // Phase 2: remove each node.
        for wid in order {
            let Some(node) = self.nodes.remove(wid) else {
                continue;
            };

            for eff in node.owned_effects {
                dispose_effect(eff);
            }

            match node.payload {
                WidgetPayload::CustomGpuSurface { state, .. } => {
                    if let Some(handle) = state.into_inner().handle
                        && released.insert(handle)
                    {
                        self.pending_texture_releases.push(handle);
                    }
                }
                WidgetPayload::Image { state, .. } => {
                    let st = state.into_inner();
                    if let Some(handle) = st.handle {
                        let should_release = if let Some(key) = st.source_key {
                            if let Some((_, refs)) = self.decoded_tex_cache.get_mut(&key) {
                                *refs = refs.saturating_sub(1);
                                if *refs == 0 {
                                    self.decoded_tex_cache.remove(&key);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                true
                            }
                        } else {
                            true
                        };
                        if should_release && released.insert(handle) {
                            self.pending_texture_releases.push(handle);
                        }
                    }
                }
                WidgetPayload::Icon { state, .. } => {
                    if let Some(handle) = state.into_inner().handle
                        && released.insert(handle)
                    {
                        self.pending_texture_releases.push(handle);
                    }
                }
                WidgetPayload::Button { icon_state, .. } => {
                    if let Some(handle) = icon_state.into_inner().handle
                        && released.insert(handle)
                    {
                        self.pending_texture_releases.push(handle);
                    }
                }
                WidgetPayload::VirtualizedList { state } => {
                    for (_, (handle, _, _)) in state.into_inner().art_cache {
                        if released.insert(handle) {
                            self.pending_texture_releases.push(handle);
                        }
                    }
                }
                _ => {}
            }

            self.layout_to_widget.remove(&node.layout_node);
            self.layout.remove(node.layout_node);
            self.scroll_ids.retain(|&x| x != wid);
            self.tab_ids.retain(|&x| x != wid);
            self.traffic_ids.retain(|&x| x != wid);
            self.custom_surface_ids.retain(|&x| x != wid);
            self.split_ids.retain(|&x| x != wid);
            self.virtual_list_ids.retain(|&x| x != wid);
            self.switcher_ids.retain(|&x| x != wid);
            self.dirty_layout_roots.retain(|&x| x != wid);
        }
    }

    /// Runs a full layout pass for a root of `width` by `height` logical pixels.
    ///
    /// The root's layout style is temporarily overridden with fixed pixel
    /// dimensions, all textual leaves are measured with `text`, the resulting
    /// rectangles are written back to node bounds, and pending dirty-layout
    /// markers are cleared. This is the layout step used by the benchmark
    /// harness and by [`WidgetArena::bench_sync_and_layout`] when a full pass
    /// is required. It does nothing when no root is mounted.
    pub fn bench_layout_pass(&mut self, width: f32, height: f32, text: &mut TextEngine) {
        let Some(root) = self.root else {
            return;
        };
        let root_lnode = self.nodes[root].layout_node;
        let mut rstyle = self.nodes[root].layout_style;
        rstyle.width = Length::Px(width);
        rstyle.height = Length::Px(height);
        self.layout.set_style(root_lnode, rstyle);

        let nodes = &mut self.nodes;
        let lmap = &self.layout_to_widget;
        let text_engine = text;
        self.layout
            .compute_with_measure(root_lnode, width, height, |lnode, _, _, avail_w, _| {
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
            });

        let rects = self.layout.collect_logical_rects(root_lnode);
        for (&lnode, &rect) in rects {
            if let Some(&wid) = self.layout_to_widget.get(&lnode)
                && let Some(node) = self.nodes.get_mut(wid)
            {
                node.bounds = rect;
            }
        }
        self.needs_full_layout = false;
        for &id in &self.dirty_layout_roots {
            if let Some(n) = self.nodes.get_mut(id) {
                n.dirty_layout_queued = false;
            }
        }
        self.dirty_layout_roots.clear();
    }

    /// Drains pending dirty notifications and lays out only what changed.
    ///
    /// This mirrors the state synchronization performed by
    /// [`WidgetWindow`] before painting: reactive text updates are applied,
    /// [`Show`] and [`ForEach`] children are reconciled, and dirty layout
    /// boundaries are recomputed with `text`. A full pass is used when the
    /// tree is marked dirty at the root or more than 32 layout roots are
    /// pending; otherwise only those roots are laid out.
    pub fn bench_sync_and_layout(&mut self, width: f32, height: f32, text: &mut TextEngine) {
        let dirty_batch: Vec<(WidgetId, DirtyFlags)> =
            std::mem::take(&mut *self.dirty_queue.borrow_mut());
        let mut foreach_dirty: Vec<WidgetId> = Vec::new();
        let mut show_dirty: Vec<WidgetId> = Vec::new();
        for (id, flags) in dirty_batch {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let layout_node = node.layout_node;
            let is_foreach = matches!(node.payload, WidgetPayload::ForEach { .. });
            let is_show = matches!(node.payload, WidgetPayload::Show { .. });

            if flags.layout {
                self.layout.mark_dirty(layout_node);
                let mut cur = id;
                while let Some(n) = self.nodes.get(cur) {
                    if n.is_relayout_boundary || n.parent.is_none() {
                        break;
                    }
                    if let Some(p) = n.parent {
                        cur = p;
                    } else {
                        break;
                    }
                }
                if Some(cur) == self.root {
                    self.needs_full_layout = true;
                } else if let Some(n) = self.nodes.get_mut(cur)
                    && !n.dirty_layout_queued
                {
                    n.dirty_layout_queued = true;
                    self.dirty_layout_roots.push(cur);
                }
            }

            if is_foreach && (flags.state || flags.layout) {
                foreach_dirty.push(id);
            }
            if is_show && (flags.state || flags.layout) {
                show_dirty.push(id);
            }
            if let Some(node) = self.nodes.get_mut(id) {
                match &mut node.payload {
                    WidgetPayload::Text {
                        block,
                        pending: Some(cell),
                        ..
                    } => {
                        let text_val = cell.borrow().clone();
                        block.set_text(text, &text_val);
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
                            text_block.set_text(text, &text_val);
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
                            text_block.set_text(text, &text_val);
                        }
                    }
                    _ => {}
                }
            }
        }

        for id in show_dirty {
            let (cond, active, mut when_true, mut when_false) =
                if let Some(node) = self.nodes.get_mut(id) {
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
                    .nodes
                    .get(id)
                    .map(|n| n.children.clone())
                    .unwrap_or_default();
                for child in old_children {
                    self.unmount_subtree(child);
                }
                let new_child = if next {
                    when_true.as_mut().map(|f| self.mount_node(f(), text))
                } else {
                    when_false.as_mut().map(|f| self.mount_node(f(), text))
                };
                let children: Vec<WidgetId> = new_child.into_iter().collect();
                for &child in &children {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }
                let child_lnodes: Vec<LayoutNode> = children
                    .iter()
                    .filter_map(|cid| self.nodes.get(*cid).map(|n| n.layout_node))
                    .collect();
                if let Some(node) = self.nodes.get_mut(id) {
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
                    self.layout.set_children(lnode, &child_lnodes);
                    self.layout.mark_dirty(lnode);
                }
                self.needs_full_layout = true;
            } else if let Some(node) = self.nodes.get_mut(id)
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
            let reconcile_fn = if let Some(node) = self.nodes.get_mut(id) {
                if let WidgetPayload::ForEach { reconcile } = &mut node.payload {
                    Some(std::mem::replace(reconcile, Box::new(|_, _, _| {})))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(mut reconcile_fn) = reconcile_fn {
                reconcile_fn(self, text, id);
                if let Some(node) = self.nodes.get_mut(id)
                    && let WidgetPayload::ForEach { reconcile } = &mut node.payload
                {
                    *reconcile = reconcile_fn;
                }
                self.needs_full_layout = true;
            }
        }

        if self.needs_full_layout || self.dirty_layout_roots.len() > 32 {
            self.bench_layout_pass(width, height, text);
        } else if !self.dirty_layout_roots.is_empty() {
            let roots: Vec<WidgetId> = std::mem::take(&mut self.dirty_layout_roots);
            for boundary_id in roots {
                if let Some(n) = self.nodes.get_mut(boundary_id) {
                    n.dirty_layout_queued = false;
                }
                let Some(boundary_node) = self.nodes.get(boundary_id) else {
                    continue;
                };
                let boundary_lnode = boundary_node.layout_node;
                let bw = boundary_node.bounds.width.max(1.0);
                let bh = boundary_node.bounds.height.max(1.0);
                let origin_x = boundary_node.bounds.x;
                let origin_y = boundary_node.bounds.y;

                let nodes = &mut self.nodes;
                let lmap = &self.layout_to_widget;
                let text_engine: &mut TextEngine = &mut *text;
                self.layout.compute_subtree_with_measure(
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
                let slice =
                    self.layout
                        .collect_logical_rects_into(boundary_lnode, origin_x, origin_y);
                self.rect_buf.clear();
                self.rect_buf.extend_from_slice(slice);
                for i in 0..self.rect_buf.len() {
                    let (lnode, rect) = self.rect_buf[i];
                    if let Some(&wid) = self.layout_to_widget.get(&lnode)
                        && let Some(node) = self.nodes.get_mut(wid)
                    {
                        node.bounds = rect;
                    }
                }
            }
        }
    }

    /// Advances every mounted [`VirtualizedList`] by `delta_y` logical pixels.
    ///
    /// The scroll offset is wrapped into the list's scrollable range and
    /// snapped to the spring, then the pooled row text blocks covering the new
    /// visible window are rebuilt with the list's row builder and measured
    /// with `text`. No layout or paint work is performed here.
    pub fn bench_scroll_virtual_step(&mut self, delta_y: f32, text: &mut TextEngine) {
        for &vid in &self.virtual_list_ids {
            if let Some(node) = self.nodes.get(vid)
                && let WidgetPayload::VirtualizedList { state } = &node.payload
            {
                let mut st = state.borrow_mut();
                let stride = st.item_height + st.gap;
                let total_h = (st.item_count as f32 * stride - st.gap).max(0.0);
                let max_scroll = (total_h - node.bounds.height).max(0.0);
                let next = (st.scroll_y.value + delta_y) % max_scroll.max(1.0);
                st.scroll_y.snap_to(next);

                let first = (next / stride).floor().max(0.0) as usize;
                let visible_n = (node.bounds.height / stride).ceil() as usize + 2;
                let last = (first + visible_n).min(st.item_count);
                let pool_len = st.pool.len().max(1);
                for idx in first..last {
                    let slot_idx = idx % pool_len;
                    if st.pool[slot_idx].cached_index != Some(idx) {
                        let row_data = (st.row_builder)(idx);
                        let slot = &mut st.pool[slot_idx];
                        slot.cached_index = Some(idx);
                        slot.title.set_text(text, &row_data.title);
                        slot.subtitle.set_text(text, &row_data.subtitle);
                        slot.badge.set_text(text, &row_data.badge);
                        slot.badge_width = slot.badge.measure(text, None).0;
                    }
                }
            }
        }
    }

    fn mount_node(&mut self, element: Element, text: &mut TextEngine) -> WidgetId {
        match element {
            Element::Container(c) => {
                let child_ids: Vec<WidgetId> = c
                    .children
                    .into_iter()
                    .map(|ch| self.mount_node(ch, text))
                    .collect();
                let child_layout_nodes: Vec<LayoutNode> = child_ids
                    .iter()
                    .map(|id| self.nodes[*id].layout_node)
                    .collect();
                let is_boundary = matches!(c.layout.width, Length::Px(_))
                    && matches!(c.layout.height, Length::Px(_));
                let lnode = self.layout.new_container(c.layout, &child_layout_nodes);
                let radius_cell = Rc::new(Cell::new(c.radius));
                let material_cell = Rc::new(Cell::new(c.material));

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: is_boundary,
                    dirty_layout_queued: false,
                    children: child_ids.clone(),
                    layout_node: lnode,
                    layout_style: c.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: c.tooltip,
                    context_menu: c.context_menu,
                    payload: WidgetPayload::Container {
                        material: material_cell.clone(),
                        radius: radius_cell.clone(),
                        is_drag_region: c.is_drag_region,
                        hover_fill: c.hover_fill,
                        on_click: c.on_click,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                for &child in &child_ids {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }

                if let Some(sig) = c.radius_signal {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        if let Some(v) = sig.try_get() {
                            radius_cell.set(v);
                            push_dirty(
                                &q,
                                id,
                                DirtyFlags {
                                    state: false,
                                    layout: false,
                                    paint: true,
                                },
                            );
                        }
                    });
                    self.nodes[id].owned_effects.push(eff);
                }

                if let Some(sig) = c.material_signal {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        if let Some(m) = sig.try_get() {
                            material_cell.set(m);
                            push_dirty(
                                &q,
                                id,
                                DirtyFlags {
                                    state: false,
                                    layout: false,
                                    paint: true,
                                },
                            );
                        }
                    });
                    self.nodes[id].owned_effects.push(eff);
                }
                id
            }
            Element::Glass(g) => self.mount_node(Element::Container(g.inner), text),
            Element::Image(img) => {
                let (source_key, decoded) = match &img.source {
                    ImageSource::File(p) => (None, load_image_file(p).ok().map(Arc::new)),
                    ImageSource::Svg {
                        data,
                        width,
                        height,
                    } => (
                        None,
                        rasterize_svg(data, *width, *height).ok().map(Arc::new),
                    ),
                    ImageSource::Decoded(d) => (Some(Arc::as_ptr(d) as usize), Some(d.clone())),
                };
                let lnode = self.layout.new_leaf(img.layout);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: img.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: img.tooltip,
                    context_menu: img.context_menu,
                    payload: WidgetPayload::Image {
                        state: RefCell::new(ImageState {
                            source_key,
                            width: decoded.as_ref().map(|d| d.width).unwrap_or(1),
                            height: decoded.as_ref().map(|d| d.height).unwrap_or(1),
                            image: decoded,
                            handle: None,
                        }),
                        radius: img.radius,
                        tint: img.tint,
                        shadow: img.shadow,
                        fit: img.fit,
                        on_click: img.on_click,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                if matches!(
                    self.nodes[id].payload,
                    WidgetPayload::Image {
                        on_click: Some(_),
                        ..
                    }
                ) {
                    self.focus.focus_order.push(id);
                }
                id
            }
            Element::Icon(ic) => {
                let px = (ic.size * 2.0).round().max(1.0) as u32;
                let decoded = rasterize_svg(&ic.svg, px, px).ok();
                let lstyle = LayoutStyle {
                    width: Length::Px(ic.size),
                    height: Length::Px(ic.size),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: ic.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Icon {
                        state: RefCell::new(IconState {
                            image: decoded,
                            handle: None,
                        }),
                        tone: ic.tone,
                        color: ic.color,
                        size: ic.size,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                id
            }
            Element::Text(t) => {
                let lstyle = LayoutStyle::default();
                let lnode = self.layout.new_leaf(lstyle);
                let shared = Rc::new(RefCell::new(String::new()));
                let is_reactive = matches!(t.source, TextSource::Reactive(_));
                let initial = match &t.source {
                    TextSource::Static(s) => s.clone(),
                    TextSource::Reactive(_) => String::new(),
                };
                let mut block = TextBlock::new(text, &initial, t.style);
                block.set_single_line_ellipsis(t.single_line);

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Text {
                        block,
                        pending: if is_reactive {
                            Some(shared.clone())
                        } else {
                            None
                        },
                        tone: t.tone,
                    },
                });
                self.layout_to_widget.insert(lnode, id);

                if let TextSource::Reactive(mut f) = t.source {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        let next = f();
                        let mut slot = shared.borrow_mut();
                        if *slot != next {
                            *slot = next;
                            push_dirty(
                                &q,
                                id,
                                DirtyFlags {
                                    state: true,
                                    layout: true,
                                    paint: true,
                                },
                            );
                        }
                    });
                    self.nodes[id].owned_effects.push(eff);
                }
                id
            }
            Element::Button(b) => {
                let icon_image = b.svg_icon.as_ref().and_then(|svg| {
                    let px = (b.icon_size * 2.0).round().max(1.0) as u32;
                    rasterize_svg(svg, px, px).ok()
                });
                let label_block = TextBlock::new(
                    text,
                    &b.label,
                    TextStyle {
                        font_size: 13.0,
                        line_height: 18.0,
                        color: Color::from_rgba8(29, 29, 31, 255),
                        weight: FontWeight::Medium,
                    },
                );
                let lstyle = LayoutStyle {
                    width: b.width,
                    height: b.height,
                    padding: if b.svg_icon.is_some() && b.label.is_empty() {
                        Edges::all(0.0)
                    } else {
                        Edges::xy(14.0, 6.0)
                    },
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: b.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Button {
                        label: label_block,
                        label_empty: b.label.is_empty(),
                        accent: b.accent,
                        variant: b.variant,
                        icon_state: RefCell::new(IconState {
                            image: icon_image,
                            handle: None,
                        }),
                        icon_size: b.icon_size,
                        pill: b.pill,
                        on_click: b.on_click,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::Switch(sw) => {
                let lstyle = LayoutStyle {
                    width: Length::Px(38.0),
                    height: Length::Px(22.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: sw.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Switch { signal: sw.signal },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = sw.signal.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: false,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::Slider(sl) => {
                let lstyle = LayoutStyle {
                    width: Length::Px(sl.width),
                    height: Length::Px(24.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: sl.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Slider {
                        signal: sl.signal,
                        min: sl.min,
                        max: sl.max,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = sl.signal.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: false,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::ProgressBar(pb) => {
                let lstyle = LayoutStyle {
                    width: pb.width,
                    height: Length::Px(pb.height),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::ProgressBar {
                        signal: pb.signal,
                        color: pb.color,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = pb.signal.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: false,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                id
            }
            Element::TextInput(ti) => {
                let lstyle = LayoutStyle {
                    width: ti.width,
                    height: Length::Px(34.0),
                    padding: Edges::xy(12.0, 7.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let current_val = ti.signal.try_get().unwrap_or_default();
                let cursor = current_val.len();
                let text_block = TextBlock::new(
                    text,
                    &current_val,
                    TextStyle {
                        font_size: 13.5,
                        line_height: 18.0,
                        color: Color::from_rgba8(28, 28, 32, 255),
                        weight: FontWeight::Regular,
                    },
                );
                let mut char_offsets = Vec::new();
                text_block.collect_char_offsets(&mut char_offsets);

                let placeholder_block = TextBlock::new(
                    text,
                    &ti.placeholder,
                    TextStyle {
                        font_size: 13.5,
                        line_height: 18.0,
                        color: Color::from_rgba8(142, 142, 147, 255),
                        weight: FontWeight::Regular,
                    },
                );

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: ti.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::TextInput {
                        signal: ti.signal,
                        material: ti.material,
                        cursor,
                        selection: None,
                        is_selecting: false,
                        text_block,
                        placeholder_block,
                        char_offsets,
                        last_synced_text: current_val,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = ti.signal.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: true,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::TextArea(ta) => {
                let lstyle = LayoutStyle {
                    width: ta.width,
                    height: ta.height,
                    padding: Edges::all(10.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let current_val = ta.signal.try_get().unwrap_or_default();
                let text_block = TextBlock::new(
                    text,
                    &current_val,
                    TextStyle {
                        font_size: 13.5,
                        line_height: 18.0,
                        color: Color::from_rgba8(28, 28, 32, 255),
                        weight: FontWeight::Regular,
                    },
                );
                let placeholder_block = TextBlock::new(
                    text,
                    &ta.placeholder,
                    TextStyle {
                        font_size: 13.5,
                        line_height: 18.0,
                        color: Color::from_rgba8(142, 142, 147, 255),
                        weight: FontWeight::Regular,
                    },
                );
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::TextArea {
                        signal: ta.signal,
                        material: ta.material,
                        cursor: current_val.len(),
                        selection: None,
                        is_selecting: false,
                        text_block,
                        placeholder_block,
                        last_synced_text: current_val,
                        scroll_y: 0.0,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = ta.signal.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: true,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::Switcher(sw) => {
                let initial_active = sw.signal.try_get().unwrap_or(0);
                let child_ids: Vec<WidgetId> = sw
                    .children
                    .into_iter()
                    .map(|ch| self.mount_node(ch, text))
                    .collect();
                let child_layout_nodes: Vec<LayoutNode> = child_ids
                    .iter()
                    .map(|id| self.nodes[*id].layout_node)
                    .collect();
                let lstyle = sw.layout;
                let lnode = self.layout.new_container(lstyle, &child_layout_nodes);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: child_ids.clone(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Switcher {
                        signal: sw.signal,
                        children: child_ids.clone(),
                        last_active: initial_active,
                        prev_idx: None,
                        transition: Spring::new(1.0, 320.0, 28.0),
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                for &child in &child_ids {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }

                for (i, &cid) in child_ids.iter().enumerate() {
                    if let Some(node) = self.nodes.get_mut(cid) {
                        node.layout_style.hidden = i != initial_active;
                        let style = node.layout_style;
                        let ln = node.layout_node;
                        self.layout.set_style(ln, style);
                    }
                }

                let q = self.dirty_queue.clone();
                let children_for_eff = child_ids.clone();
                let eff = create_effect(move || {
                    let active = sw.signal.try_get().unwrap_or(0);
                    // We'll mark layout dirty; actual hidden toggling happens in sync_switchers.
                    let _ = active;
                    let _ = &children_for_eff;
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: true,
                            layout: true,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.switcher_ids.push(id);
                id
            }
            Element::Show(mut sh) => {
                let condition = sh.condition;
                let initial = condition.try_get().unwrap_or(false);
                let mut active_child = None;
                let child_id = if initial {
                    sh.when_true.as_mut().map(|f| {
                        let cid = self.mount_node(f(), text);
                        active_child = Some(cid);
                        cid
                    })
                } else {
                    sh.when_false.as_mut().map(|f| {
                        let cid = self.mount_node(f(), text);
                        active_child = Some(cid);
                        cid
                    })
                };
                let children: Vec<WidgetId> = child_id.into_iter().collect();
                let child_lnodes: Vec<LayoutNode> = children
                    .iter()
                    .filter_map(|cid| self.nodes.get(*cid).map(|n| n.layout_node))
                    .collect();
                let lnode = self.layout.new_container(sh.layout, &child_lnodes);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: children.clone(),
                    layout_node: lnode,
                    layout_style: sh.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Show {
                        condition,
                        active: initial,
                        when_true: sh.when_true,
                        when_false: sh.when_false,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                for &child in &children {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = condition.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: true,
                            layout: true,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                id
            }
            Element::ForEach(mut fe) => {
                let lnode = self.layout.new_container(fe.layout, &[]);
                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    parent: None,
                    is_relayout_boundary: matches!(
                        (fe.layout.width, fe.layout.height),
                        (Length::Px(_), Length::Px(_))
                    ),
                    dirty_layout_queued: false,
                    layout_node: lnode,
                    layout_style: fe.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::ForEach {
                        reconcile: Box::new(|_, _, _| {}),
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                let eff = (fe.bind_signal)(self.dirty_queue.clone(), id);
                self.nodes[id].owned_effects.push(eff);
                // Populate initial children immediately on mount!
                (fe.reconcile)(self, text, id);
                if let Some(node) = self.nodes.get_mut(id)
                    && let WidgetPayload::ForEach { reconcile } = &mut node.payload
                {
                    *reconcile = fe.reconcile;
                }
                id
            }
            Element::Tabs(tb) => {
                let count = tb.items.len().max(1);
                let lstyle = LayoutStyle {
                    width: tb.width,
                    min_width: Length::Px(count as f32 * 68.0),
                    height: Length::Px(30.0),
                    padding: Edges::all(2.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);

                let mut labels = Vec::with_capacity(tb.items.len());
                let mut label_widths = Vec::with_capacity(tb.items.len());
                for it in &tb.items {
                    let mut block = TextBlock::new(
                        text,
                        it,
                        TextStyle {
                            font_size: 12.5,
                            line_height: 16.0,
                            color: Color::from_rgba8(28, 28, 32, 255),
                            weight: FontWeight::Medium,
                        },
                    );
                    let (w, _) = block.measure(text, None);
                    labels.push(block);
                    label_widths.push(w);
                }

                let initial_tab = tb.signal.try_get().unwrap_or(0);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: tb.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Tabs {
                        signal: tb.signal,
                        items: tb.items,
                        labels,
                        label_widths,
                        material: tb.material,
                        pill_spring: Spring::new(initial_tab as f32, 320.0, 26.0),
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = tb.signal.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: false,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                self.tab_ids.push(id);
                id
            }
            Element::Dropdown(dd) => {
                let lstyle = LayoutStyle {
                    width: dd.width,
                    height: Length::Px(32.0),
                    padding: Edges::xy(12.0, 6.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let labels = dd
                    .items
                    .iter()
                    .map(|it| {
                        TextBlock::new(
                            text,
                            it,
                            TextStyle {
                                font_size: 13.0,
                                line_height: 18.0,
                                color: Color::from_rgba8(28, 28, 32, 255),
                                weight: FontWeight::Medium,
                            },
                        )
                    })
                    .collect();
                let selected_labels = dd
                    .items
                    .iter()
                    .map(|it| {
                        TextBlock::new(
                            text,
                            it,
                            TextStyle {
                                font_size: 13.0,
                                line_height: 18.0,
                                color: Color::from_rgba8(255, 255, 255, 255),
                                weight: FontWeight::Semibold,
                            },
                        )
                    })
                    .collect();
                let chevron = TextBlock::new(
                    text,
                    "v",
                    TextStyle {
                        font_size: 11.0,
                        line_height: 14.0,
                        color: Color::from_rgba8(120, 120, 128, 255),
                        weight: FontWeight::Bold,
                    },
                );

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: dd.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Dropdown {
                        signal: dd.signal,
                        items: dd.items,
                        labels,
                        selected_labels,
                        material: dd.material,
                        is_open: false,
                        chevron,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = dd.signal.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: false,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::CustomGpuSurface(cs) => {
                let lnode = self.layout.new_leaf(cs.layout);
                let radius_cell = Rc::new(Cell::new(cs.radius));
                let needs_redraw = Rc::new(Cell::new(true));
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: cs.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: cs.tooltip,
                    context_menu: cs.context_menu,
                    payload: WidgetPayload::CustomGpuSurface {
                        radius: radius_cell.clone(),
                        animate_signal: cs.animate_signal,
                        needs_redraw: needs_redraw.clone(),
                        resolution_scale: cs.resolution_scale,
                        state: RefCell::new(CustomSurfaceState {
                            pass: cs.pass,
                            handle: None,
                            view: None,
                            size: (0, 0),
                        }),
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                self.custom_surface_ids.push(id);

                if let Some(sig) = cs.radius_signal {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        if let Some(v) = sig.try_get() {
                            radius_cell.set(v);
                            push_dirty(
                                &q,
                                id,
                                DirtyFlags {
                                    state: false,
                                    layout: false,
                                    paint: true,
                                },
                            );
                        }
                    });
                    self.nodes[id].owned_effects.push(eff);
                }
                for binder in cs.dirty_binders {
                    let eff = binder(self.dirty_queue.clone(), id, needs_redraw.clone());
                    self.nodes[id].owned_effects.push(eff);
                }
                id
            }
            Element::TrafficLights(_) => {
                let lstyle = LayoutStyle {
                    width: Length::Px(56.0),
                    height: Length::Px(16.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::TrafficLights { hovered: false },
                });
                self.layout_to_widget.insert(lnode, id);
                self.traffic_ids.push(id);
                id
            }
            Element::ScrollView(sv) => {
                let child_ids: Vec<WidgetId> = sv
                    .children
                    .into_iter()
                    .map(|ch| self.mount_node(ch, text))
                    .collect();
                let child_lnodes: Vec<LayoutNode> = child_ids
                    .iter()
                    .map(|id| self.nodes[*id].layout_node)
                    .collect();

                let content_lstyle = LayoutStyle {
                    direction: FlexDir::Column,
                    gap: sv.gap,
                    flex_shrink: 0.0,
                    ..Default::default()
                };
                let content_lnode = self.layout.new_container(content_lstyle, &child_lnodes);
                let content_id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: child_ids.clone(),
                    layout_node: content_lnode,
                    layout_style: content_lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Container {
                        material: Rc::new(Cell::new(Material::solid(Color::TRANSPARENT))),
                        radius: Rc::new(Cell::new(0.0)),
                        is_drag_region: false,
                        hover_fill: None,
                        on_click: None,
                    },
                });
                self.layout_to_widget.insert(content_lnode, content_id);
                for &child in &child_ids {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(content_id);
                    }
                }

                let viewport_lstyle = LayoutStyle {
                    direction: FlexDir::Column,
                    flex_grow: 1.0,
                    ..Default::default()
                };
                let viewport_lnode = self.layout.new_container(viewport_lstyle, &[content_lnode]);

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: vec![content_id],
                    layout_node: viewport_lnode,
                    layout_style: viewport_lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::ScrollView {
                        scroll_y: Spring::new(0.0, 260.0, 24.0),
                        content_height: 0.0,
                        content_id,
                    },
                });
                self.layout_to_widget.insert(viewport_lnode, id);
                if let Some(node) = self.nodes.get_mut(content_id) {
                    node.parent = Some(id);
                }
                self.scroll_ids.push(id);
                id
            }
            Element::SplitPane(sp) => {
                let first_elem = sp
                    .first
                    .map(|b| *b)
                    .unwrap_or_else(|| Spacer::flex().into_element());
                let second_elem = sp
                    .second
                    .map(|b| *b)
                    .unwrap_or_else(|| Spacer::flex().into_element());

                let first_id = self.mount_node(first_elem, text);
                let second_id = self.mount_node(second_elem, text);

                let init_ratio = sp
                    .ratio
                    .try_get()
                    .unwrap_or(0.3)
                    .clamp(sp.min_ratio, sp.max_ratio);

                let is_h = sp.direction == SplitDirection::Horizontal;

                let mut first_style = self.nodes[first_id].layout_style;
                if is_h {
                    first_style.width = Length::Percent(init_ratio);
                    first_style.height = Length::Auto;
                } else {
                    first_style.height = Length::Percent(init_ratio);
                    first_style.width = Length::Auto;
                }
                first_style.flex_grow = 0.0;
                first_style.flex_shrink = 0.0;
                self.nodes[first_id].layout_style = first_style;
                self.layout
                    .set_style(self.nodes[first_id].layout_node, first_style);

                let div_style = LayoutStyle {
                    width: if is_h { Length::Px(1.0) } else { Length::Auto },
                    height: if is_h { Length::Auto } else { Length::Px(1.0) },
                    flex_shrink: 0.0,
                    ..Default::default()
                };
                let div_lnode = self.layout.new_leaf(div_style);
                let divider_id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: div_lnode,
                    layout_style: div_style,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Spacer,
                });
                self.layout_to_widget.insert(div_lnode, divider_id);

                let mut second_style = self.nodes[second_id].layout_style;
                second_style.flex_grow = 1.0;
                second_style.flex_shrink = 1.0;
                self.nodes[second_id].layout_style = second_style;
                self.layout
                    .set_style(self.nodes[second_id].layout_node, second_style);

                let parent_style = LayoutStyle {
                    direction: if is_h { FlexDir::Row } else { FlexDir::Column },
                    flex_grow: 1.0,
                    ..Default::default()
                };
                let child_lnodes = [
                    self.nodes[first_id].layout_node,
                    div_lnode,
                    self.nodes[second_id].layout_node,
                ];
                let parent_lnode = self.layout.new_container(parent_style, &child_lnodes);

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: vec![first_id, divider_id, second_id],
                    layout_node: parent_lnode,
                    layout_style: parent_style,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::SplitPane {
                        direction: sp.direction,
                        ratio: sp.ratio,
                        min_ratio: sp.min_ratio,
                        max_ratio: sp.max_ratio,
                        first_id,
                        divider_id,
                        divider_color: sp.divider_color,
                    },
                });
                self.layout_to_widget.insert(parent_lnode, id);
                for &child in &[first_id, divider_id, second_id] {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }
                self.split_ids.push(id);

                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = sp.ratio.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: true,
                            layout: true,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                id
            }
            Element::VirtualizedList(vl) => {
                let lstyle = LayoutStyle {
                    flex_grow: 1.0,
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);

                let pool = (0..40)
                    .map(|_| VirtualRowSlot {
                        cached_index: None,
                        title: TextBlock::new(
                            text,
                            "",
                            TextStyle {
                                font_size: 13.0,
                                line_height: 18.0,
                                color: Color::from_rgba8(29, 29, 31, 255),
                                weight: FontWeight::Medium,
                            },
                        ),
                        subtitle: TextBlock::new(
                            text,
                            "",
                            TextStyle {
                                font_size: 11.5,
                                line_height: 15.0,
                                color: Color::from_rgba8(110, 110, 115, 255),
                                weight: FontWeight::Regular,
                            },
                        ),
                        badge: TextBlock::new(
                            text,
                            "",
                            TextStyle {
                                font_size: 12.0,
                                line_height: 16.0,
                                color: Color::from_rgba8(110, 110, 115, 255),
                                weight: FontWeight::Medium,
                            },
                        ),
                        badge_width: 0.0,
                    })
                    .collect();

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::VirtualizedList {
                        state: RefCell::new(VirtualListState {
                            item_count: vl.item_count,
                            item_height: vl.item_height,
                            gap: vl.gap,
                            flat: vl.flat,
                            columns_mode: vl.columns_mode,
                            accent: vl.accent,
                            scroll_y: Spring::new(0.0, 260.0, 24.0),
                            hovered_row: None,
                            selected: vl.selected,
                            on_select: vl.on_select,
                            row_builder: vl.row_builder,
                            row_art: vl.row_art,
                            art_cache: HashMap::new(),
                            pool,
                        }),
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                self.virtual_list_ids.push(id);

                if let Some(sel) = vl.selected {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        let _ = sel.try_get();
                        push_dirty(
                            &q,
                            id,
                            DirtyFlags {
                                state: false,
                                layout: false,
                                paint: true,
                            },
                        );
                    });
                    self.nodes[id].owned_effects.push(eff);
                }
                id
            }
            Element::Divider(d) => {
                let lstyle = LayoutStyle {
                    height: Length::Px(1.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Divider { color: d.color },
                });
                self.layout_to_widget.insert(lnode, id);
                id
            }
            Element::Spacer(s) => {
                let lstyle = LayoutStyle {
                    flex_grow: s.grow,
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Spacer,
                });
                self.layout_to_widget.insert(lnode, id);
                id
            }
        }
    }
}

impl Default for WidgetArena {
    fn default() -> Self {
        Self::new()
    }
}

