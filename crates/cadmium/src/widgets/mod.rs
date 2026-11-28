//! Declarative widgets and the element tree that backs a Cadmium window.
//!
//! Widgets in this module are lightweight, immutable *descriptions* built with a
//! chainable builder API. Each builder consumes and returns `Self`, so a UI is
//! written as an expression tree rather than as mutable widget objects:
//!
//! ```
//! use cadmium::prelude::*;
//!
//! let ui = Container::new()
//!     .padding(Edges::all(16.0))
//!     .gap(10.0)
//!     .child(Text::new("Settings").size(20.0).tone(TextTone::Primary))
//!     .child(
//!         Row::new()
//!             .gap(8.0)
//!             .child(Button::new("Cancel").ghost())
//!             .child(Button::new("Save").primary()),
//!     );
//!
//! // All builders implement [`IntoElement`]; the conversion is explicit at the
//! // boundary where the tree is handed to the framework.
//! let element: Element = ui.into_element();
//! ```
//!
//! # The element tree and conversion
//!
//! [`Element`] is the single enum that represents every kind of widget in a
//! mounted tree. Calling [`IntoElement::into_element`] erases the concrete
//! builder type into that enum, which lets containers accept heterogeneous
//! children through [`Container::child`], [`Container::children`], and the
//! similar methods on other containers. Most builders also have an
//! `IntoElement` implementation that returns their corresponding [`Element`]
//! variant directly.
//!
//! # Ownership and mounting
//!
//! The builder tree is not the live UI. [`WidgetArena`] owns the mounted
//! widget nodes, their layout nodes, focus state, reactive effects, and cached
//! GPU textures. [`WidgetArena::mount`] iteratively converts an [`Element`]
//! tree into arena nodes and returns the root [`WidgetId`]; replacing a tree
//! with another call to `mount` discards the previous one and releases its
//! resources. Most applications never call this directly: [`WidgetWindow`]
//! implements [`crate::window::WindowDelegate`], invokes a user-supplied build
//! closure, and mounts the resulting element automatically.
//!
//! # Reactive widgets
//!
//! Widgets that display mutable state — [`Switch`], [`Slider`],
//! [`ProgressBar`], [`TextInput`], [`TextArea`], [`Tabs`], [`Dropdown`],
//! [`SplitPane`], and others — store a [`crate::reactive::Signal`] instead of a
//! snapshot value. Mounting such a widget installs a reactive effect in the
//! arena that marks the node dirty when the signal changes. Structural widgets
//! build dynamic subtrees the same way: [`Show`] switches between two builder
//! closures, [`ForEach`] reconciles a signal-driven list by stable key, and
//! [`Switcher`] hides all but the selected child. [`Text::reactive`] produces a
//! text widget whose contents are refreshed by a closure.
//!
//! # Container composition
//!
//! [`Container`] is the general-purpose layout and decoration node: it owns
//! children, flex settings, material, border radius, tooltip, context menu,
//! and click handling. [`Column`] and [`Row`] are aliases/convenience entries
//! for the two flex directions, [`Stack`] builds an unconfigured container,
//! and [`ScrollView`], [`SplitPane`], and [`Switcher`] add their own child
//! layout policies. Widgets that wrap a container, such as [`GlassPanel`],
//! forward builder calls to that inner container so they compose with the
//! same layout engine.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::accessibility::{A11yAction, A11yNodeRecord, A11yRole, SemanticTree};
use crate::animation::Spring;
use crate::assets::{DecodedImage, cover_uv_rect, load_image_file, rasterize_svg};
use crate::core::{Color, CornerRadii, Rect};
use crate::create_signal;
use crate::debug::FrameProfiler;
use crate::events::{FocusManager, Key, Modifiers, WidgetId};
use crate::layout::{
    AlignItems, Edges, FlexDir, Inset, JustifyContent, LayoutEngine, LayoutNode, LayoutStyle,
    Length, PositionMode, snap_rect_to_pixels,
};
use crate::platform::Clipboard;
use crate::reactive::{EffectId, Signal, create_effect, dispose_effect};
use crate::renderer::{ATLAS_FORMAT, GlassQuad, ImageQuad, SdfQuad, TextureHandle};
use crate::style::{MacTheme, Material, Shadow, ThemeMode};
use crate::text::{FontWeight, TextBlock, TextEngine, TextStyle};
use crate::window::{CursorStyle, FrameContext, WindowCommand, WindowDelegate};
use slotmap::SlotMap;

include!("element.rs");
include!("containers.rs");
include!("card.rs");
include!("badge.rs");
include!("link.rs");
include!("checkbox.rs");
include!("table.rs");
include!("command_palette.rs");
include!("tree_view.rs");
include!("dock_area.rs");
include!("menu_bar.rs");
include!("toast.rs");
include!("grid.rs");
include!("dx.rs");
include!("modal.rs");
include!("accordion.rs");
include!("segmented_control.rs");
include!("avatar.rs");
include!("radio_button.rs");
include!("text_widget.rs");
include!("breakpoint.rs");
include!("wrap.rs");
include!("media.rs");
include!("controls.rs");
include!("views.rs");
include!("arena.rs");
include!("window_impl.rs");
include!("paint.rs");
include!("tests.rs");
