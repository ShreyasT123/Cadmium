/// Application callbacks that drive one native window.
///
/// Implement this trait when drawing a window directly; for a widget-based UI
/// use [`crate::widgets::WidgetWindow`] instead. [`Self::render`] is the only
/// required method. Every other method has a default implementation that
/// records no state and reports "not handled" (`false`), no scheduled redraw
/// (`None`), or [`WindowCommand::None`].
///
/// Returning `true` from an input or tick callback requests a redraw of that
/// window. The delegate is boxed into [`WindowSpec`] and owned by [`App`], so
/// implementations must be `'static`. Pointer positions are logical pixels;
/// the event loop converts them from physical surface coordinates using the
/// window's scale factor.
///
/// # Examples
///
/// ```
/// use cadmium::events::{Key, Modifiers};
/// use cadmium::window::{CursorStyle, FrameContext, WindowDelegate};
///
/// struct Counter {
///     presses: usize,
/// }
///
/// impl WindowDelegate for Counter {
///     fn on_key(&mut self, key: &Key, _text: Option<&str>, _modifiers: &Modifiers) -> bool {
///         if *key == Key::Enter {
///             self.presses += 1;
///             true
///         } else {
///             false
///         }
///     }
///
///     fn cursor_style(&self) -> CursorStyle {
///         CursorStyle::Pointer
///     }
///
///     fn render(&mut self, _ctx: &mut FrameContext<'_>) {}
/// }
/// ```
pub trait WindowDelegate: 'static {
    /// Called once after the window and its text engine are ready, before the
    /// first frame.
    ///
    /// Use this to load fonts or build long-lived text blocks. The default
    /// implementation does nothing.
    fn init(&mut self, _text: &mut TextEngine) {}

    /// Called once after the shared GPU context is available, before the first
    /// frame.
    ///
    /// Use this to upload textures owned by the delegate. The default
    /// implementation does nothing.
    fn init_gpu(&mut self, _gpu: &mut GpuContext) {}

    /// Called once per rendered frame with the time since the last tick in
    /// seconds.
    ///
    /// Return `true` to keep animating: the app requests another redraw
    /// immediately. Drawing belongs in [`Self::render`], not here. The event
    /// loop clamps `dt` to `0.0001..=0.05` seconds. The default returns `false`.
    fn tick(&mut self, _dt: f32) -> bool {
        false
    }

    /// Returns a time at which a single redraw should be requested, if any.
    ///
    /// The event loop waits until the earliest deadline across all windows and
    /// requests a redraw when it is reached. Return [`None`] when nothing is
    /// scheduled or [`Self::tick`] is already animating. The default returns
    /// `None`.
    fn next_redraw_at(&self) -> Option<Instant> {
        None
    }

    /// Called when the pointer moves over the window.
    ///
    /// `x` and `y` are logical pixel coordinates. When the pointer leaves the
    /// window, the app calls this with `(-1.0, -1.0)`. Return `true` to request
    /// a redraw. The default returns `false`.
    fn on_pointer_move(&mut self, _x: f32, _y: f32) -> bool {
        false
    }

    /// Called when the primary (left) mouse button is pressed.
    ///
    /// `x` and `y` are logical pixel coordinates. The first return value is
    /// whether the event was handled (and therefore whether to redraw); the
    /// second is a window-level action to apply. If both are `false` and
    /// [`WindowCommand::None`], [`Self::is_drag_region`] is consulted to start
    /// a native window drag. The default reports unhandled with no command.
    fn on_pointer_down(&mut self, _x: f32, _y: f32) -> (bool, WindowCommand) {
        (false, WindowCommand::None)
    }

    /// Called when the secondary (right) mouse button is pressed.
    ///
    /// `x` and `y` are logical pixel coordinates. Return `true` to request a
    /// redraw. The default returns `false`.
    fn on_secondary_down(&mut self, _x: f32, _y: f32) -> bool {
        false
    }

    /// Called when the primary (left) mouse button is released.
    ///
    /// Return `true` to request a redraw. The default returns `false`.
    fn on_pointer_up(&mut self) -> bool {
        false
    }

    /// Called for mouse-wheel input.
    ///
    /// Line-based wheel deltas are multiplied by `40.0` before being passed
    /// in; pixel-based deltas are forwarded unchanged. Return `true` to
    /// request a redraw. The default returns `false`.
    fn on_scroll(&mut self, _delta_x: f32, _delta_y: f32) -> bool {
        false
    }

    /// Called for a pressed, non-synthetic keyboard key.
    ///
    /// `key` is the translated [`Key`], `text` is the printable text supplied
    /// with the key event (if any), and `modifiers` is the event loop's current
    /// modifier state. Return `true` to request a redraw. The default returns
    /// `false`.
    fn on_key(&mut self, _key: &Key, _text: Option<&str>, _modifiers: &Modifiers) -> bool {
        false
    }

    /// Returns whether a primary-button press at these logical coordinates
    /// should start a native window drag.
    ///
    /// The app checks this only when [`Self::on_pointer_down`] did not handle
    /// the press and returned [`WindowCommand::None`]. It is primarily useful
    /// for custom title bars in borderless windows. The default returns
    /// `false`.
    fn is_drag_region(&self, _x: f32, _y: f32) -> bool {
        false
    }

    /// Returns the cursor shape to use over the window.
    ///
    /// Consulted on pointer movement; [`CursorStyle`] lists the supported
    /// shapes. Resize edges of borderless windows override this value. The
    /// default is [`CursorStyle::Default`].
    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Default
    }

    /// Called when an assistive technology enables or disables accessibility.
    ///
    /// Implementations can use this to prepare or release expensive semantic
    /// data. The default implementation does nothing.
    fn set_a11y_active(&mut self, _active: bool) {}

    /// Returns the current semantic tree for AccessKit, if available.
    ///
    /// The app supplies it to AccessKit after presenting a frame while
    /// accessibility is active. Returning [`None`] leaves the tree unchanged.
    /// The default returns `None`.
    fn semantic_tree(&self) -> Option<SemanticTree> {
        None
    }

    /// Called when an assistive technology requests an action on a widget.
    ///
    /// Return `true` if the action was applied: the app then flushes reactive
    /// effects and requests a redraw. The default returns `false`.
    fn on_semantic_action(&mut self, _action: A11yAction) -> bool {
        false
    }

    /// Records this frame's primitives into `ctx`.
    ///
    /// This is the only required method. Draw by calling the recording methods
    /// on [`FrameContext`]; the app submits the resulting draw list after this
    /// method returns.
    fn render(&mut self, ctx: &mut FrameContext<'_>);

    /// Called when the window gains or loses keyboard focus.
    ///
    /// Return `true` to request a redraw. Losing focus also clears the event
    /// loop's modifier state. The default returns `false`.
    fn on_window_focus(&mut self, _focused: bool) -> bool {
        false
    }
}

/// Description of a window to be created by [`App::run`].
///
/// A spec is a builder input, not a live window: [`App::add_window`] queues it,
/// and the window itself is created when the event loop first resumes. The
/// delegate owns the window's application state for its whole lifetime.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let spec = WindowSpec {
///     title: "Settings".into(),
///     width: 640.0,
///     height: 480.0,
///     min_width: None,
///     min_height: None,
///     borderless: false,
///     clear_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
///     delegate: Box::new(WidgetWindow::new(|| Text::new("Settings").into_element())),
/// };
/// let _app = App::new().add_window(spec);
/// ```
pub struct WindowSpec {
    /// Title displayed in the window's platform title bar.
    pub title: String,
    /// Initial inner width in logical pixels (device-independent pixels).
    pub width: f64,
    /// Initial inner height in logical pixels (device-independent pixels).
    pub height: f64,
    /// Minimum inner width in logical pixels, enforced by the platform window.
    ///
    /// `None` leaves the platform default in place. Setting this prevents a tiled
    /// or resized window from shrinking below a usable content width.
    pub min_width: Option<f64>,
    /// Minimum inner height in logical pixels, enforced by the platform window.
    ///
    /// `None` leaves the platform default in place. Setting this prevents a tiled
    /// or resized window from shrinking below a usable content height.
    pub min_height: Option<f64>,
    /// Whether to create the window without platform decorations.
    ///
    /// Borderless windows have six-logical-pixel resize edges. Because the
    /// platform title bar is absent, custom controls should use
    /// [`WindowDelegate::is_drag_region`] and [`WindowCommand::Close`] for
    /// dragging and closing.
    pub borderless: bool,
    /// Background color used when clearing the window's render target.
    pub clear_color: Color,
    /// The delegate that owns this window's state and receives its callbacks.
    pub delegate: Box<dyn WindowDelegate>,
}

