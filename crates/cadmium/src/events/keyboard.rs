/// A keyboard key delivered to [`crate::window::WindowDelegate::on_key`].
///
/// The window runner translates `winit`'s logical key into these values for
/// pressed, non-synthetic key events. Printable input appears as
/// [`Self::Character`], while named navigation and editing keys have dedicated
/// variants. Printable text associated with a key press is also passed
/// separately as `on_key`'s `text` argument.
///
/// # Examples
///
/// ```
/// use cadmium::events::{Key, Modifiers};
///
/// fn accepts(key: &Key, mods: &Modifiers) -> bool {
///     *key == Key::Enter && mods.ctrl
/// }
///
/// assert!(accepts(&Key::Enter, &Modifiers { ctrl: true, ..Default::default() }));
/// assert!(!accepts(&Key::Enter, &Modifiers::default()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// Printable text produced by the key press, as reported by `winit`.
    ///
    /// The string may contain more than one character when supplied by an
    /// input method.
    Character(String),
    /// Backspace key.
    Backspace,
    /// Delete key.
    Delete,
    /// Enter or Return key.
    Enter,
    /// Tab key.
    Tab,
    /// Escape key.
    Escape,
    /// Space bar, distinct from a [`Self::Character`] containing a space.
    Space,
    /// Left arrow key.
    ArrowLeft,
    /// Right arrow key.
    ArrowRight,
    /// Up arrow key.
    ArrowUp,
    /// Down arrow key.
    ArrowDown,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Any key not represented by another variant.
    Unknown,
}

/// A mouse button named by a pointer event.
///
/// Used by [`UiEvent::PointerDown`] and [`UiEvent::PointerUp`]. The window
/// delegate has separate callbacks for primary and secondary presses and for
/// primary release; see [`crate::window::WindowDelegate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    /// Primary (usually left) button.
    Left,
    /// Secondary (usually right) button.
    Right,
    /// Middle button, often bound to wheel clicks.
    Middle,
}

/// Keyboard modifier keys held during an input event.
///
/// The window runner derives this state from `winit`'s modifier-change events
/// and clears it when a window loses focus. The current value is passed to
/// [`crate::window::WindowDelegate::on_key`] and appears in
/// [`UiEvent::KeyDown`]. All flags default to `false`.
///
/// # Examples
///
/// ```
/// use cadmium::events::Modifiers;
///
/// let mods = Modifiers {
///     ctrl: true,
///     shift: true,
///     ..Default::default()
/// };
/// assert!(mods.ctrl && mods.shift);
/// assert!(!mods.alt && !mods.super_key);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Whether either Shift key is held.
    pub shift: bool,
    /// Whether either Control key is held.
    pub ctrl: bool,
    /// Whether either Alt key is held.
    pub alt: bool,
    /// Whether a super key is held (Command on macOS, Windows key on Windows).
    pub super_key: bool,
}

/// Whether an event should continue to further handlers after the current one.
///
/// Handlers that only observe an event return [`Self::Continue`]; handlers that
/// consume it return [`Self::Stop`]. The value is plain data: dispatch code
/// decides how to interpret it.
///
/// # Examples
///
/// ```
/// use cadmium::events::EventPropagation;
///
/// fn on_click(consumed: bool) -> EventPropagation {
///     if consumed {
///         EventPropagation::Stop
///     } else {
///         EventPropagation::Continue
///     }
/// }
///
/// assert_eq!(on_click(true), EventPropagation::Stop);
/// assert_eq!(on_click(false), EventPropagation::Continue);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPropagation {
    /// Let subsequent handlers observe the event.
    Continue,
    /// Do not pass the event to further handlers.
    Stop,
}

