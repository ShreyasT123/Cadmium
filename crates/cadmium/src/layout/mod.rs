use std::collections::HashMap;

use taffy::prelude::*;

use crate::core::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutNode(NodeId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDir {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    Start,
    End,
    Center,
    Stretch,
}

impl From<AlignItems> for taffy::style::AlignItems {
    fn from(a: AlignItems) -> Self {
        match a {
            AlignItems::Start => taffy::style::AlignItems::START,
            AlignItems::End => taffy::style::AlignItems::END,
            AlignItems::Center => taffy::style::AlignItems::CENTER,
            AlignItems::Stretch => taffy::style::AlignItems::STRETCH,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    Start,
    End,
    Center,
    SpaceBetween,
}

impl From<JustifyContent> for taffy::style::JustifyContent {
    fn from(j: JustifyContent) -> Self {
        match j {
            JustifyContent::Start => taffy::style::JustifyContent::START,
            JustifyContent::End => taffy::style::JustifyContent::END,
            JustifyContent::Center => taffy::style::JustifyContent::CENTER,
            JustifyContent::SpaceBetween => taffy::style::JustifyContent::SPACE_BETWEEN,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Auto,
    Px(f32),
    Percent(f32),
}

impl From<Length> for Dimension {
    fn from(len: Length) -> Self {
        match len {
            Length::Auto => Dimension::auto(),
            Length::Px(v) => Dimension::length(v),
            Length::Percent(p) => Dimension::percent(p),
        }
    }
}

impl From<Length> for LengthPercentageAuto {
    fn from(len: Length) -> Self {
        match len {
            Length::Auto => LengthPercentageAuto::auto(),
            Length::Px(v) => LengthPercentageAuto::length(v),
            Length::Percent(p) => LengthPercentageAuto::percent(p),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edges {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Edges {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn all(v: f32) -> Self {
        Self {
            left: v,
            right: v,
            top: v,
            bottom: v,
        }
    }

    pub const fn xy(x: f32, y: f32) -> Self {
        Self {
            left: x,
            right: x,
            top: y,
            bottom: y,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LayoutStyle {
    pub direction: FlexDir,
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub padding: Edges,
    pub gap: f32,
    pub align_items: Option<AlignItems>,
    pub justify_content: Option<JustifyContent>,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            direction: FlexDir::Column,
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            padding: Edges::ZERO,
            gap: 0.0,
            align_items: None,
            justify_content: None,
        }
    }
}

impl From<LayoutStyle> for Style {
    fn from(s: LayoutStyle) -> Self {
        let base = Style::default();
        Style {
            display: Display::Flex,
            flex_direction: match s.direction {
                FlexDir::Row => FlexDirection::Row,
                FlexDir::Column => FlexDirection::Column,
            },
            size: Size {
                width: s.width.into(),
                height: s.height.into(),
            },
            min_size: Size {
                width: s.min_width.into(),
                height: s.min_height.into(),
            },
            flex_grow: s.flex_grow,
            flex_shrink: s.flex_shrink,
            padding: taffy::Rect {
                left: LengthPercentage::length(s.padding.left),
                right: LengthPercentage::length(s.padding.right),
                top: LengthPercentage::length(s.padding.top),
                bottom: LengthPercentage::length(s.padding.bottom),
            },
            gap: Size {
                width: LengthPercentage::length(s.gap),
                height: LengthPercentage::length(s.gap),
            },
            align_items: s.align_items.map(Into::into).unwrap_or(base.align_items),
            justify_content: s
                .justify_content
                .map(Into::into)
                .unwrap_or(base.justify_content),
            ..base
        }
    }
}

pub struct LayoutEngine {
    tree: TaffyTree<()>,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            tree: TaffyTree::new(),
        }
    }

    pub fn new_leaf(&mut self, style: LayoutStyle) -> LayoutNode {
        let id = self
            .tree
            .new_leaf(style.into())
            .expect("Taffy leaf allocation");
        LayoutNode(id)
    }

    pub fn new_container(&mut self, style: LayoutStyle, children: &[LayoutNode]) -> LayoutNode {
        let child_ids: Vec<NodeId> = children.iter().map(|n| n.0).collect();
        let id = self
            .tree
            .new_with_children(style.into(), &child_ids)
            .expect("Taffy container allocation");
        LayoutNode(id)
    }

    pub fn set_style(&mut self, node: LayoutNode, style: LayoutStyle) {
        let _ = self.tree.set_style(node.0, style.into());
    }

    pub fn set_children(&mut self, parent: LayoutNode, children: &[LayoutNode]) {
        let child_ids: Vec<NodeId> = children.iter().map(|n| n.0).collect();
        let _ = self.tree.set_children(parent.0, &child_ids);
    }

    pub fn mark_dirty(&mut self, node: LayoutNode) {
        let _ = self.tree.mark_dirty(node.0);
    }

    pub fn remove(&mut self, node: LayoutNode) {
        let _ = self.tree.remove(node.0);
    }

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

    pub fn collect_logical_rects(&self, root: LayoutNode) -> HashMap<LayoutNode, Rect> {
        let mut out = HashMap::new();
        self.walk_rects(root.0, 0.0, 0.0, &mut out);
        out
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

pub fn snap_rect_to_pixels(rect: Rect, scale: f32) -> Rect {
    let x0 = (rect.x * scale).round();
    let y0 = (rect.y * scale).round();
    let x1 = ((rect.x + rect.width) * scale).round();
    let y1 = ((rect.y + rect.height) * scale).round();
    Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taffy_row_layout_and_pixel_snap() {
        let mut eng = LayoutEngine::new();
        let a = eng.new_leaf(LayoutStyle {
            width: Length::Px(100.0),
            height: Length::Px(40.0),
            ..Default::default()
        });
        let b = eng.new_leaf(LayoutStyle {
            flex_grow: 1.0,
            height: Length::Px(40.0),
            ..Default::default()
        });
        let root = eng.new_container(
            LayoutStyle {
                direction: FlexDir::Row,
                width: Length::Px(400.0),
                height: Length::Px(40.0),
                gap: 10.0,
                ..Default::default()
            },
            &[a, b],
        );

        eng.compute_with_measure(root, 400.0, 40.0, |_, _, _, _, _| (0.0, 0.0));
        let rects = eng.collect_logical_rects(root);
        assert_eq!(rects[&a], Rect::new(0.0, 0.0, 100.0, 40.0));
        assert_eq!(rects[&b], Rect::new(110.0, 0.0, 290.0, 40.0));

        let snapped = snap_rect_to_pixels(Rect::new(10.3, 10.3, 99.8, 20.0), 1.5);
        assert_eq!(snapped.x, 15.0);
        assert_eq!(snapped.width, 150.0);
    }
}
