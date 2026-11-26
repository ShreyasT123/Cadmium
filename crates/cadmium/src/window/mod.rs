//! Native windows, the application event loop, and per-frame drawing callbacks.
//!
//! Cadmium uses `winit` for platform windows and input, and `wgpu` through
//! [`crate::renderer`] for drawing. [`App`] owns the event loop and shares one
//! [`crate::renderer::GpuContext`] across its windows. Each window is described
//! by a [`WindowSpec`], whose [`WindowDelegate`] receives input callbacks and
//! records drawing commands into a [`FrameContext`].
//!
//! # Starting an application
//!
//! Add one or more window specifications with [`App::add_window`], then call
//! [`App::run`]. The built-in [`crate::widgets::WidgetWindow`] delegate mounts a
//! tree of [`crate::widgets`] into a window, so most applications only need to
//! build elements:
//!
//! ```no_run
//! use cadmium::prelude::*;
//!
//! let app = App::new().add_window(WindowSpec {
//!     title: "Hello".into(),
//!     width: 480.0,
//!     height: 320.0,
//!     borderless: false,
//!     clear_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
//!     delegate: Box::new(WidgetWindow::new(|| {
//!         Text::new("Hello, Cadmium").size(20.0).into_element()
//!     })),
//! });
//! app.run().unwrap();
//! ```
//!
//! Custom rendering is possible by implementing [`WindowDelegate`] directly;
//! [`FrameContext`] forwards primitives to a [`crate::renderer::DrawList`]
//! without requiring direct access to the GPU device. See [`crate::renderer`]
//! for the coordinate conventions of each primitive.
//!
//! # Input, focus, and commands
//!
//! Physical pointer coordinates from `winit` are divided by the window's scale
//! factor before they reach delegate callbacks, so delegates work in logical
//! pixels. The [`crate::events::Modifiers`] state is tracked by the event loop,
//! including clearing it when a window loses focus, and is passed to
//! [`WindowDelegate::on_key`] along with a [`crate::events::Key`]. Input
//! callbacks return `true` when they handled the event and therefore need a
//! redraw. [`WindowDelegate::on_pointer_down`] can additionally return a
//! [`WindowCommand`] to close or minimize the window, toggle maximization, or
//! redraw every window. [`WindowDelegate::cursor_style`] selects the cursor
//! icon; borderless-window resize edges take precedence with their own resize
//! cursors.
//!
//! # Multiple windows and scheduling
//!
//! Several [`WindowSpec`]s may be queued before [`App::run`]. They are created
//! when the event loop resumes and all use the same GPU context. Closing a
//! window removes it from the app; when the last window closes, the event loop
//! exits. [`App::dispatcher`] provides a [`crate::reactive::UiDispatcher`] that
//! lets background threads run closures on the event-loop thread and wake it.
//!
//! A delegate requests animation by returning `true` from
//! [`WindowDelegate::tick`], or a one-shot redraw by returning an `Instant`
//! from [`WindowDelegate::next_redraw_at`]. Reactive effects are flushed before
//! each frame and after handled input, and the event loop waits until the
//! earliest scheduled redraw when no animation is active.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{CursorIcon, ResizeDirection, Window, WindowId},
};

use crate::accessibility::{self, A11yAction, SemanticTree};
use crate::core::{CadmiumError, Color};
use crate::events::{Key, Modifiers};
use crate::reactive::{self, UiDispatcher};
use crate::renderer::{DrawList, GlassQuad, GpuContext, ImageQuad, SdfQuad, WindowRenderTarget};
use crate::text::TextEngine;

include!("types.rs");
include!("delegate.rs");
include!("app.rs");
include!("handler.rs");
include!("runner.rs");
