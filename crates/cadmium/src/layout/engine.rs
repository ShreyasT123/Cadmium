/// Owns a Taffy flexbox tree and computes rectangles for its nodes.
///
/// The engine is reusable: nodes may be added, restyled, reparented, and removed between
/// frames, and its scratch buffers are retained to avoid reallocating when rectangles are
/// collected. It is normally owned by [`crate::widgets::WidgetArena`], which maps each
/// [`LayoutNode`] back to a widget and applies the computed bounds.
///
/// All input sizes and all rectangles returned by this engine are in logical pixels.
pub struct LayoutEngine {
    tree: TaffyTree<()>,
    rect_scratch: Vec<(LayoutNode, Rect)>,
    /// Reusable scratch HashMap so `collect_logical_rects` doesn't allocate
    /// on every relayout.  Cleared in-place, not dropped and re-created.
    rect_map_scratch: HashMap<LayoutNode, Rect>,
}

impl LayoutEngine {
    /// Creates an empty layout tree with fresh scratch buffers.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::layout::{LayoutEngine, LayoutStyle, Length};
    ///
    /// let mut engine = LayoutEngine::new();
    /// let node = engine.new_leaf(LayoutStyle {
    ///     width: Length::Px(10.0),
    ///     height: Length::Px(10.0),
    ///     ..Default::default()
    /// });
    /// engine.compute_with_measure(node, 10.0, 10.0, |_, _, _, _, _| (0.0, 0.0));
    /// assert!(engine.collect_logical_rects(node).contains_key(&node));
    /// ```
    pub fn new() -> Self {
        Self {
            tree: TaffyTree::new(),
            rect_scratch: Vec::new(),
            rect_map_scratch: HashMap::new(),
        }
    }

    /// Adds a childless node with the given style and returns its handle.
    ///
    /// Leaf nodes are the nodes for which [`Self::compute_with_measure`] calls the
    /// measurement callback. Add the returned handle to a container with
    /// [`Self::new_container`] or [`Self::set_children`].
    ///
    /// # Panics
    ///
    /// Panics if Taffy cannot allocate the node.
    pub fn new_leaf(&mut self, style: LayoutStyle) -> LayoutNode {
        let id = self
            .tree
            .new_leaf(style.into())
            .expect("Taffy leaf allocation");
        LayoutNode(id)
    }

    /// Adds a node with `children` and returns its handle.
    ///
    /// All children must be nodes created by this same engine, in the desired stacking
    /// and layout order. The style controls how those children are arranged.
    ///
    /// # Panics
    ///
    /// Panics if Taffy cannot allocate the container.
    pub fn new_container(&mut self, style: LayoutStyle, children: &[LayoutNode]) -> LayoutNode {
        let child_ids: Vec<NodeId> = children.iter().map(|n| n.0).collect();
        let id = self
            .tree
            .new_with_children(style.into(), &child_ids)
            .expect("Taffy container allocation");
        LayoutNode(id)
    }

    /// Replaces a node's [`LayoutStyle`].
    ///
    /// Call [`Self::mark_dirty`] afterwards to invalidate cached layout for the node.
    /// Errors from Taffy are ignored, so passing a stale handle from another engine is a
    /// no-op.
    pub fn set_style(&mut self, node: LayoutNode, style: LayoutStyle) {
        let _ = self.tree.set_style(node.0, style.into());
    }

    /// Replaces a parent node's children with `children`, in order.
    ///
    /// All children must come from this engine. Errors from Taffy are ignored, so passing
    /// a stale parent or child handle is a no-op; call [`Self::mark_dirty`] afterwards to
    /// invalidate cached layout.
    pub fn set_children(&mut self, parent: LayoutNode, children: &[LayoutNode]) {
        let child_ids: Vec<NodeId> = children.iter().map(|n| n.0).collect();
        let _ = self.tree.set_children(parent.0, &child_ids);
    }

    /// Marks a node and its descendants as needing a fresh layout computation.
    ///
    /// Call this after changing a style or children, or when measured content (such as
    /// text) changes. Errors from Taffy are ignored, so a stale handle is a no-op.
    pub fn mark_dirty(&mut self, node: LayoutNode) {
        let _ = self.tree.mark_dirty(node.0);
    }

    /// Removes a node and its descendants from the tree.
    ///
    /// Any [`LayoutNode`] handles for the removed nodes become stale and should be
    /// discarded. Errors from Taffy are ignored, so removing an unknown node is a no-op.
    pub fn remove(&mut self, node: LayoutNode) {
        let _ = self.tree.remove(node.0);
    }

    /// Computes layout for the subtree rooted at `root` within `width` × `height` of
    /// available logical-pixel space.
    ///
    /// Taffy calls `measure` for leaf nodes whose size cannot be resolved from the
    /// [`LayoutStyle`] alone. The callback receives:
    ///
    /// 1. the node being measured,
    /// 2. its known width, if any,
    /// 3. its known height, if any,
    /// 4. the available width, if definite,
    /// 5. the available height, if definite.
    ///
    /// It returns the measured `(width, height)` in logical pixels. Widgets use this to
    /// measure text and icon-bearing labels; returning `(0.0, 0.0)` is appropriate for
    /// leaves with purely style-driven sizes.
    ///
    /// # Panics
    ///
    /// Panics if Taffy fails to compute the layout.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::layout::{FlexDir, LayoutEngine, LayoutStyle, Length};
    ///
    /// let mut engine = LayoutEngine::new();
    /// let label = engine.new_leaf(LayoutStyle::default());
    /// let root = engine.new_container(
    ///     LayoutStyle {
    ///         direction: FlexDir::Row,
    ///         width: Length::Px(200.0),
    ///         height: Length::Px(50.0),
    ///         ..Default::default()
    ///     },
    ///     &[label],
    /// );
    ///
    /// // A text-like leaf reports its measured size in logical pixels.
    /// engine.compute_with_measure(root, 200.0, 50.0, |_, _, _, _, _| (80.0, 20.0));
    /// let rects = engine.collect_logical_rects(root);
    /// assert_eq!(rects[&label].width, 80.0);
    /// ```
    pub fn compute_with_measure<F>(
        &mut self,
        root: LayoutNode,
        width: f32,
        height: f32,
        mut measure: F,
    ) where
        F: FnMut(LayoutNode, Option<f32>, Option<f32>, Option<f32>, Option<f32>) -> (f32, f32),
    {
        let available = Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        };

        self.tree
            .compute_layout_with_measure(root.0, available, |inputs, node_id, _ctx, style| {
                taffy::compute::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, avail| {
                        let avail_w = match avail.width {
                            AvailableSpace::Definite(w) => Some(w),
                            _ => None,
                        };
                        let avail_h = match avail.height {
                            AvailableSpace::Definite(h) => Some(h),
                            _ => None,
                        };
                        let (w, h) = measure(
                            LayoutNode(node_id),
                            known.width,
                            known.height,
                            avail_w,
                            avail_h,
                        );
                        Size {
                            width: w,
                            height: h,
                        }
                    },
                )
            })
            .expect("Taffy layout computation");
    }

    /// Computes layout rooted at a relayout boundary (`boundary`) without walking ancestors.
    ///
    /// This is equivalent to [`Self::compute_with_measure`] but is intended for partial
    /// relayouts: `width` and `height` should be the boundary's current logical-pixel
    /// bounds, and `boundary` should be a node whose ancestors have already been laid out.
    /// The widget arena uses this for subtrees marked dirty by reactive updates.
    ///
    /// # Panics
    ///
    /// Panics if Taffy fails to compute the layout; see [`Self::compute_with_measure`].
    pub fn compute_subtree_with_measure<F>(
        &mut self,
        boundary: LayoutNode,
        width: f32,
        height: f32,
        measure: F,
    ) where
        F: FnMut(LayoutNode, Option<f32>, Option<f32>, Option<f32>, Option<f32>) -> (f32, f32),
    {
        self.compute_with_measure(boundary, width, height, measure);
    }

    /// Collects computed rectangles relative to `origin_x`/`origin_y` into a reusable
    /// scratch buffer and returns them as a borrowed slice.
    ///
    /// Each entry pairs a node with its logical-pixel [`Rect`], with the root first and
    /// descendants following in depth-first order. A node that has not been laid out yet
    /// contributes an empty (zero-sized) rectangle. The returned slice borrows internal
    /// scratch storage and is invalidated by
    /// the next call to this method, [`Self::collect_logical_rects`], or any other
    /// `&mut self` method; copy the entries if they must outlive that call.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Rect;
    /// use cadmium::layout::{LayoutEngine, LayoutStyle, Length};
    ///
    /// let mut engine = LayoutEngine::new();
    /// let child = engine.new_leaf(LayoutStyle {
    ///     width: Length::Px(10.0),
    ///     height: Length::Px(20.0),
    ///     ..Default::default()
    /// });
    /// engine.compute_with_measure(child, 10.0, 20.0, |_, _, _, _, _| (0.0, 0.0));
    /// let rects = engine.collect_logical_rects_into(child, 100.0, 200.0);
    /// assert_eq!(rects.len(), 1);
    /// assert_eq!(rects[0], (child, Rect::new(100.0, 200.0, 10.0, 20.0)));
    /// ```
    pub fn collect_logical_rects_into(
        &mut self,
        root: LayoutNode,
        origin_x: f32,
        origin_y: f32,
    ) -> &[(LayoutNode, Rect)] {
        self.rect_scratch.clear();
        let mut out = std::mem::take(&mut self.rect_scratch);
        self.walk_rects_vec(root.0, origin_x, origin_y, &mut out);
        self.rect_scratch = out;
        &self.rect_scratch
    }

    fn walk_rects_vec(
        &self,
        node: NodeId,
        parent_x: f32,
        parent_y: f32,
        out: &mut Vec<(LayoutNode, Rect)>,
    ) {
        if let Ok(layout) = self.tree.layout(node) {
            let x = parent_x + layout.location.x;
            let y = parent_y + layout.location.y;
            out.push((
                LayoutNode(node),
                Rect::new(x, y, layout.size.width, layout.size.height),
            ));
            if let Ok(children) = self.tree.children(node) {
                for child in children {
                    self.walk_rects_vec(child, x, y, out);
                }
            }
        }
    }

    /// Collects computed rectangles relative to the root's origin `(0.0, 0.0)` into a
    /// reusable map keyed by [`LayoutNode`].
    ///
    /// The values are logical-pixel [`Rect`]s; the widget arena assigns them as widget
    /// bounds. A node that has not been laid out yet contributes an empty (zero-sized)
    /// rectangle. The returned map borrows internal scratch storage and is invalidated by
    /// the next call to this method, [`Self::collect_logical_rects_into`], or any other
    /// `&mut self` method.
    pub fn collect_logical_rects(&mut self, root: LayoutNode) -> &HashMap<LayoutNode, Rect> {
        self.rect_map_scratch.clear();
        // Temporarily take the scratch map out to avoid a simultaneous borrow
        // of `self` (needed by `walk_rects`) and `self.rect_map_scratch`.
        let mut map = std::mem::take(&mut self.rect_map_scratch);
        self.walk_rects(root.0, 0.0, 0.0, &mut map);
        self.rect_map_scratch = map;
        &self.rect_map_scratch
    }

    fn walk_rects(
        &self,
        node: NodeId,
        parent_x: f32,
        parent_y: f32,
        out: &mut HashMap<LayoutNode, Rect>,
    ) {
        if let Ok(layout) = self.tree.layout(node) {
            let x = parent_x + layout.location.x;
            let y = parent_y + layout.location.y;
            out.insert(
                LayoutNode(node),
                Rect::new(x, y, layout.size.width, layout.size.height),
            );
            if let Ok(children) = self.tree.children(node) {
                for child in children {
                    self.walk_rects(child, x, y, out);
                }
            }
        }
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Rounds a logical-pixel rectangle to whole device pixels.
///
/// The rectangle's left/top and right/bottom corners are each multiplied by `scale`
/// (typically the window's device-pixel ratio) and rounded to the nearest integer, with
/// halves rounded away from zero. The result is a device-pixel rectangle whose width and
/// height are derived from the rounded corners and clamped to be non-negative. Widget
/// rendering calls this before submitting geometry to [`crate::renderer`], keeping
/// borders aligned to the pixel grid at fractional scale factors.
///
/// # Examples
///
/// ```
/// use cadmium::core::Rect;
/// use cadmium::layout::snap_rect_to_pixels;
///
/// let snapped = snap_rect_to_pixels(Rect::new(10.3, 10.3, 99.8, 20.0), 1.5);
/// assert_eq!(snapped, Rect::new(15.0, 15.0, 150.0, 30.0));
/// ```
pub fn snap_rect_to_pixels(rect: Rect, scale: f32) -> Rect {
    let x0 = (rect.x * scale).round();
    let y0 = (rect.y * scale).round();
    let x1 = ((rect.x + rect.width) * scale).round();
    let y1 = ((rect.y + rect.height) * scale).round();
    Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

