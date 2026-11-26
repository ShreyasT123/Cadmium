//! Layout computation for [`crate::widgets`], powered by [Taffy](https://docs.rs/taffy).
//!
//! Taffy implements CSS flexbox and grid layout; this module exposes its flexbox-oriented
//! subset as a small, plain-data API. Each widget supplies a [`LayoutStyle`] describing its
//! constraints, and a [`LayoutEngine`] turns the resulting tree of [`LayoutNode`]s into
//! logical-pixel rectangles ([`crate::core::Rect`]) that hit testing and rendering consume.
//! All sizes, paddings, margins, gaps, and insets here are in **logical pixels**: conversion
//! to device pixels happens later, in [`crate::renderer`] (see [`snap_rect_to_pixels`]).
//!
//! # Core concepts
//!
//! - [`Length`] expresses a size as automatic, a fixed number of logical pixels, or a
//!   percentage of the parent's corresponding axis.
//! - [`Edges`] holds the four logical-pixel values used for [`LayoutStyle::padding`] and
//!   [`LayoutStyle::margin`], while [`Inset`] holds per-side [`Length`]s used for
//!   absolutely positioned nodes.
//! - [`FlexDir`], [`AlignItems`], and [`JustifyContent`] select the flexbox axis and how
//!   children are distributed. [`PositionMode`] selects in-flow versus absolute
//!   positioning, and [`LayoutStyle::hidden`] omits a node from layout entirely.
//! - [`LayoutNode`] is an opaque handle into a [`LayoutEngine`]'s tree. Widgets in
//!   [`crate::widgets::WidgetArena`] own one each; application code normally goes through
//!   the arena's builder methods instead of calling the engine directly.
//! - [`snap_rect_to_pixels`] rounds computed rectangles to whole device pixels so that
//!   hairlines and text stay crisp; widget rendering applies it before pushing geometry
//!   to the GPU.
//!
//! # Example
//!
//! ```
//! use cadmium::core::Rect;
//! use cadmium::layout::{
//!     FlexDir, LayoutEngine, LayoutStyle, Length, snap_rect_to_pixels,
//! };
//!
//! let mut engine = LayoutEngine::new();
//! let fixed = engine.new_leaf(LayoutStyle {
//!     width: Length::Px(100.0),
//!     height: Length::Px(40.0),
//!     ..Default::default()
//! });
//! let flexible = engine.new_leaf(LayoutStyle {
//!     flex_grow: 1.0,
//!     height: Length::Px(40.0),
//!     ..Default::default()
//! });
//! let root = engine.new_container(
//!     LayoutStyle {
//!         direction: FlexDir::Row,
//!         width: Length::Px(400.0),
//!         height: Length::Px(40.0),
//!         gap: 10.0,
//!         ..Default::default()
//!     },
//!     &[fixed, flexible],
//! );
//!
//! engine.compute_with_measure(root, 400.0, 40.0, |_, _, _, _, _| (0.0, 0.0));
//! let rects = engine.collect_logical_rects(root);
//! assert_eq!(rects[&fixed], Rect::new(0.0, 0.0, 100.0, 40.0));
//! assert_eq!(rects[&flexible], Rect::new(110.0, 0.0, 290.0, 40.0));
//!
//! let device_rect = snap_rect_to_pixels(rects[&fixed], 2.0);
//! assert_eq!(device_rect, Rect::new(0.0, 0.0, 200.0, 80.0));
//! ```

use std::collections::HashMap;

use taffy::geometry::Point;
use taffy::prelude::*;
use taffy::style::Overflow;

use crate::core::Rect;

include!("types.rs");
include!("style.rs");
include!("engine.rs");
include!("tests.rs");
