//! Accessibility support built on [AccessKit](https://accesskit.dev/).
//!
//! Cadmium describes a window's controls as a [`SemanticTree`]: a collection of
//! [`A11yNodeRecord`]s whose roles are given by [`A11yRole`]. [`crate::window::App`]
//! converts that tree into an AccessKit `TreeUpdate` when assistive technology
//! requests it, and translates incoming `ActionRequest`s into [`A11yAction`]s that
//! are delivered to [`crate::window::WindowDelegate::on_semantic_action`]. The
//! built-in [`crate::widgets::WidgetWindow`] builds a tree from its widget arena,
//! so most applications get this bridge without implementing it themselves.
//!
//! Ids in this module are [`crate::events::WidgetId`]s, reused directly as
//! AccessKit `NodeId`s so a widget keeps the same accessibility identity across
//! tree updates. Node bounds are in **physical pixels** (logical widget bounds
//! multiplied by the frame scale factor), matching the coordinate space used by
//! [`crate::renderer`].
//!
//! # Examples
//!
//! ```rust
//! use cadmium::accessibility::{A11yNodeRecord, A11yRole, SemanticTree};
//! use cadmium::core::Rect;
//! use cadmium::events::WidgetId;
//!
//! fn describe(root: WidgetId, button: WidgetId) -> SemanticTree {
//!     SemanticTree {
//!         root,
//!         focus: Some(button),
//!         nodes: vec![
//!             A11yNodeRecord {
//!                 id: root,
//!                 role: A11yRole::Window,
//!                 label: Some("Main window".into()),
//!                 value: None,
//!                 numeric_value: None,
//!                 toggled: None,
//!                 focusable: false,
//!                 clickable: false,
//!                 bounds: Rect::new(0.0, 0.0, 800.0, 600.0),
//!                 children: vec![button],
//!             },
//!             A11yNodeRecord {
//!                 id: button,
//!                 role: A11yRole::Button,
//!                 label: Some("Run".into()),
//!                 value: None,
//!                 numeric_value: None,
//!                 toggled: None,
//!                 focusable: true,
//!                 clickable: true,
//!                 bounds: Rect::new(24.0, 16.0, 96.0, 28.0),
//!                 children: vec![],
//!             },
//!         ],
//!     }
//! }
//! ```

use accesskit::{Action, ActionRequest, Node, NodeId, Role, Toggled, TreeId, TreeInfo, TreeUpdate};
use slotmap::{Key, KeyData};

use crate::core::Rect;
use crate::events::WidgetId;

include!("types.rs");
include!("bridge.rs");
include!("tests.rs");
