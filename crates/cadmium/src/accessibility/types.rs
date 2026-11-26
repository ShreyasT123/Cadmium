/// Semantic role assigned to an accessibility node.
///
/// Each variant corresponds to one AccessKit `Role` when the tree is published.
/// Use [`A11yRole::Container`] for grouping nodes without more specific meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A11yRole {
    /// The top-level window; maps to AccessKit `Window`.
    Window,
    /// A grouping node without specialized semantics; maps to `GenericContainer`.
    Container,
    /// Static, non-interactive text; maps to `Label`.
    Label,
    /// An activatable control; maps to `Button`.
    Button,
    /// A two-state control whose state is reported by [`A11yNodeRecord::toggled`];
    /// maps to `Switch`.
    Switch,
    /// A control with a numeric [`A11yNodeRecord::numeric_value`]; maps to `Slider`.
    Slider,
    /// An editable text field whose contents are reported by
    /// [`A11yNodeRecord::value`]; maps to `TextInput`.
    TextInput,
}

/// Converts a Cadmium role into the AccessKit role exposed to assistive technology.
impl From<A11yRole> for Role {
    fn from(r: A11yRole) -> Self {
        match r {
            A11yRole::Window => Role::Window,
            A11yRole::Container => Role::GenericContainer,
            A11yRole::Label => Role::Label,
            A11yRole::Button => Role::Button,
            A11yRole::Switch => Role::Switch,
            A11yRole::Slider => Role::Slider,
            A11yRole::TextInput => Role::TextInput,
        }
    }
}

/// An accessibility action requested by assistive technology.
///
/// The window bridge produces these from AccessKit `ActionRequest`s and passes
/// them to [`crate::window::WindowDelegate::on_semantic_action`]; the payload is
/// the [`crate::events::WidgetId`] to act on. Only click and focus requests are
/// translated; other AccessKit actions are ignored.
///
/// # Examples
///
/// ```rust
/// use cadmium::accessibility::A11yAction;
/// use cadmium::events::WidgetId;
///
/// fn handle(action: A11yAction, run_button: WidgetId) -> &'static str {
///     match action {
///         A11yAction::Click(id) if id == run_button => "run",
///         A11yAction::Focus(id) if id == run_button => "focus",
///         _ => "unhandled",
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A11yAction {
    /// Activate the widget as if it had been clicked.
    Click(WidgetId),
    /// Move keyboard focus to the widget.
    Focus(WidgetId),
}

/// One node in a [`SemanticTree`], describing a single widget to assistive technology.
///
/// Optional fields are simply omitted from the published AccessKit node. Bounds
/// are physical pixels; children are linked by [`crate::events::WidgetId`] and
/// should have their own records in the same tree.
#[derive(Debug, Clone)]
pub struct A11yNodeRecord {
    /// Widget that owns this node; reused as the node's accessibility identity.
    pub id: WidgetId,
    /// Semantic kind of the widget.
    pub role: A11yRole,
    /// Optional human-readable name announced for the widget.
    pub label: Option<String>,
    /// Optional text value, typically the contents of a text field.
    pub value: Option<String>,
    /// Optional numeric value, used for sliders and published as an `f64`.
    pub numeric_value: Option<f64>,
    /// Optional two-state value: `Some(true)` is published as `Toggled::True`,
    /// `Some(false)` as `Toggled::False`, and `None` leaves the state unspecified.
    pub toggled: Option<bool>,
    /// Whether the `Focus` action is advertised for this node.
    pub focusable: bool,
    /// Whether the `Click` action is advertised for this node.
    pub clickable: bool,
    /// Bounds in physical pixels; `x`/`y` are the top-left corner and `width`/
    /// `height` are the extent.
    pub bounds: Rect,
    /// Ids of child nodes, in presentation order.
    pub children: Vec<WidgetId>,
}

/// A window's accessibility hierarchy, ready to be published through AccessKit.
///
/// Returned by [`crate::window::WindowDelegate::semantic_tree`] and consumed by
/// [`crate::window::App`] when a screen reader requests a tree update.
#[derive(Debug, Clone)]
pub struct SemanticTree {
    /// Root widget of the hierarchy; also used as the focus when none is set.
    pub root: WidgetId,
    /// Currently focused widget, or `None` to use [`SemanticTree::root`].
    pub focus: Option<WidgetId>,
    /// All nodes in the hierarchy, in any order. Each record is identified by
    /// its [`A11yNodeRecord::id`].
    pub nodes: Vec<A11yNodeRecord>,
}

