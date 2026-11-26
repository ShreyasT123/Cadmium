//! Frequently used Cadmium types, collected in one namespace.
//!
//! Importing the prelude is the shortest way to start building an interface:
//!
//! ```rust
//! use cadmium::prelude::*;
//!
//! let enabled = create_signal(true);
//! let label = Text::new("Ready").size(14.0);
//! let _ = (enabled, label);
//! ```
//!
//! The prelude includes widget builders, layout values, theme types, signal
//! helpers, and the application runner. It intentionally omits renderer
//! internals and platform-specific details; import those from [`crate::renderer`]
//! and [`crate::platform`] when you need them. Select the root [`crate`] module
//! instead when you want the complete public API in scope.

pub use crate::animation::Spring;
pub use crate::core::{CadmiumError, Color, CornerRadii, Rect};
pub use crate::events::{Key, Modifiers, WidgetId};
pub use crate::layout::{AlignItems, Edges, FlexDir, Inset, JustifyContent, Length, PositionMode};
pub use crate::reactive::{
    Memo, OwnedSignal, Selector, Signal, UiDispatcher, create_effect, create_memo, create_selector,
    create_signal, flush_effects,
};
pub use crate::style::{MacTheme, Material, Shadow, ThemeMode};
pub use crate::text::{FontWeight, TextStyle};
pub use crate::widgets::{
    Accordion, Avatar, Badge, Breakpoint, Button, ButtonVariant, Card, Checkbox,
    CircularProgressIndicator, Column, CommandItem, CommandPalette, Container, CustomGpuPass,
    CustomGpuSurface, Divider, DockArea, DockGroup, DockNode, DockTab, Dropdown, Element, ForEach,
    GlassPanel, GridView, Icon, Image, ImageFit, ImageSource, IntoElement, Link, MenuAction,
    MenuBar, MenuGroup, MenuItem, Modal, ProgressBar, RadioButton, Row, ScrollView,
    SegmentedControl, ShaderCanvas, Show, Slider, Spacer, SplitDirection, SplitPane, Stack, Switch,
    Switcher, Table, TableColumn, TableRow, Tabs, Text, TextArea, TextInput, TextTone, ToastHost,
    ToastItem, ToastLevel, Toolbar, TrafficLights, TreeNode, TreeView, VirtualRow, VirtualizedList,
    WidgetWindow, Wrap,
};
pub use crate::window::{App, WindowCommand, WindowSpec, run};
