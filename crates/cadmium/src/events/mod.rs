//! Input values, widget identities, and focus bookkeeping.
//!
//! [`WidgetId`] is an opaque, generation-counted key assigned to a mounted
//! widget node. The arena in [`crate::widgets::WidgetArena`] uses these keys for
//! hit testing, focus, and accessibility, and its public
//! [`WidgetArena::mount`](crate::widgets::WidgetArena::mount) and
//! [`unmount_subtree`](crate::widgets::WidgetArena::unmount_subtree) methods
//! accept them. Keys from different arenas are not interchangeable, and a key
//! for a removed node never aliases a later node.
//!
//! # From platform input to widgets
//!
//! [`crate::window::App`] receives input from `winit`, converts pointer
//! coordinates to logical pixels, tracks [`Modifiers`], and forwards callbacks
//! to a [`crate::window::WindowDelegate`]. Printable and named key presses are
//! reported as [`Key`] values. The built-in delegate routes those callbacks to
//! the mounted widget tree; custom delegates can use the same values directly.
//! [`UiEvent`] is a plain-data description of a logical UI input event,
//! [`PointerButton`] names the mouse button involved, and [`EventPropagation`]
//! lets handlers express whether an event should continue to further handlers.
//!
//! # Focus and tab order
//!
//! [`FocusManager`] holds the currently focused, hovered, and captured widget,
//! together with the tab order used by [`FocusManager::cycle_next`] and
//! [`FocusManager::cycle_prev`]. Cycling wraps around and does not itself send
//! focus events or request redraws.
//!
//! # Examples
//!
//! ```
//! use cadmium::events::{FocusManager, Key, Modifiers};
//!
//! let mut focus = FocusManager::new();
//! assert_eq!(focus.cycle_next(), None); // no focusable widgets registered
//! assert!(!focus.set_focus(None)); // already unset
//!
//! fn is_enter(key: &Key, mods: &Modifiers) -> bool {
//!     *key == Key::Enter && !mods.shift && !mods.ctrl && !mods.alt && !mods.super_key
//! }
//! assert!(is_enter(&Key::Enter, &Modifiers::default()));
//! ```

use slotmap::new_key_type;

new_key_type! {
    /// Opaque, generation-counted key identifying a mounted widget.
    ///
    /// [`crate::widgets::WidgetArena::mount`] returns the root key, and the
    /// arena uses keys to address nodes during layout, painting, hit testing,
    /// focus changes and accessibility updates. Store a key only for as long as
    /// the corresponding node is mounted: removing a node invalidates its key,
    /// and slot reuse never produces an equal key.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::events::WidgetId;
    /// use slotmap::SlotMap;
    ///
    /// let mut nodes: SlotMap<WidgetId, ()> = SlotMap::with_key();
    /// let button = nodes.insert(());
    /// assert!(nodes.contains_key(button));
    /// nodes.remove(button);
    /// assert!(!nodes.contains_key(button));
    /// ```
    pub struct WidgetId;
}

include!("keyboard.rs");
include!("event.rs");
include!("tests.rs");
