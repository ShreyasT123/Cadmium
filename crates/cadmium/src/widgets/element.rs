const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(530);
const TOOLTIP_HOVER_DELAY: Duration = Duration::from_millis(450);

/// The kinds of work a widget needs after its observed state changes.
///
/// Dirty flags are queued by the reactive effects installed during mounting and
/// drained once per frame by the window delegate. More than one flag may be
/// set for a single widget: a changed text value, for example, affects layout
/// as well as painting.
#[derive(Debug, Clone, Copy, Default)]
pub struct DirtyFlags {
    /// Whether widget state must be re-synchronized before the next paint.
    ///
    /// State work includes updating text blocks from their signals and
    /// rebuilding dynamic children for [`Show`] and [`ForEach`].
    pub state: bool,
    /// Whether the widget's layout node must be recomputed or re-measured.
    pub layout: bool,
    /// Whether the widget's cached paint data must be regenerated.
    pub paint: bool,
}

type DirtyQueue = Rc<RefCell<Vec<(WidgetId, DirtyFlags)>>>;
type PathDrawFn = Box<dyn FnMut(&mut FrameContext<'_>)>;

fn push_dirty(queue: &DirtyQueue, id: WidgetId, flags: DirtyFlags) {
    queue.borrow_mut().push((id, flags));
}

fn snap_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn normalize_selection(s: &str, sel: Option<(usize, usize)>) -> Option<(usize, usize)> {
    sel.map(|(a, b)| (snap_char_boundary(s, a), snap_char_boundary(s, b)))
}

fn word_prev(s: &str, cursor: usize) -> usize {
    let c = snap_char_boundary(s, cursor);
    let mut it = s[..c].char_indices().rev().peekable();
    while let Some(&(_, ch)) = it.peek() {
        if ch.is_whitespace() {
            it.next();
        } else {
            break;
        }
    }
    let mut pos = 0;
    for (idx, ch) in it {
        if ch.is_whitespace() {
            pos = idx + ch.len_utf8();
            break;
        }
    }
    pos
}

fn word_next(s: &str, cursor: usize) -> usize {
    let c = snap_char_boundary(s, cursor);
    let mut it = s[c..].char_indices().peekable();
    while let Some(&(_, ch)) = it.peek() {
        if !ch.is_whitespace() {
            it.next();
        } else {
            break;
        }
    }
    while let Some(&(idx, ch)) = it.peek() {
        if ch.is_whitespace() {
            it.next();
        } else {
            return c + idx;
        }
    }
    s.len()
}

fn ensure_textarea_caret_visible(node: &mut WidgetNode) {
    if let WidgetPayload::TextArea {
        cursor,
        text_block,
        scroll_y,
        ..
    } = &mut node.payload
    {
        let (cy, _) = text_block.caret_pos_for_byte(*cursor);
        let view_h = (node.bounds.height - 20.0).max(18.0);
        if cy < *scroll_y {
            *scroll_y = cy;
        } else if cy + 18.0 > *scroll_y + view_h {
            *scroll_y = (cy + 18.0 - view_h).max(0.0);
        }
    }
}

fn find_char_index_at_x(char_offsets: &[(usize, f32)], rel_x: f32) -> usize {
    if rel_x <= 0.0 || char_offsets.is_empty() {
        return 0;
    }
    let mut best_idx = 0;
    let mut best_dist = f32::MAX;
    for &(byte_idx, x_pos) in char_offsets {
        let d = (rel_x - x_pos).abs();
        if d < best_dist {
            best_dist = d;
            best_idx = byte_idx;
        }
    }
    best_idx
}

fn _lookup_offset_x(char_offsets: &[(usize, f32)], byte_idx: usize) -> f32 {
    for &(idx, x_pos) in char_offsets {
        if idx >= byte_idx {
            return x_pos;
        }
    }
    char_offsets.last().map(|&(_, x)| x).unwrap_or(0.0)
}

fn line_index_at(s: &str, byte_idx: usize) -> usize {
    let idx = byte_idx.min(s.len());
    s.as_bytes()[..idx].iter().filter(|&&b| b == b'\n').count()
}

fn line_range(s: &str, line: usize) -> (usize, usize) {
    let mut start = 0;
    let mut current = 0;
    for (i, ch) in s.char_indices() {
        if ch == '\n' {
            if current == line {
                return (start, i);
            }
            current += 1;
            start = i + 1;
        }
    }
    if current == line {
        (start, s.len())
    } else {
        (s.len(), s.len())
    }
}

/// One entry in a widget's right-click context menu.
///
/// Menu items are collected by builder methods such as
/// [`Container::context_menu`] and stored on the mounted node until the menu
/// is opened. A menu may mix labelled commands with separators.
pub struct MenuItem {
    /// The text shown for this entry, or `None` for a separator line.
    pub label: Option<String>,
    /// The callback invoked when the entry is activated.
    ///
    /// Separators have no callback. The callback is stored as a boxed
    /// `FnMut`, so it may mutate captured state.
    pub on_select: Option<Box<dyn FnMut()>>,
}

impl MenuItem {
    /// Creates a selectable menu entry with `label` and its activation callback.
    ///
    /// The callback is invoked when the user clicks the entry in an open context
    /// menu. It must be `'static` because it is owned by the mounted widget.
    pub fn new(label: impl Into<String>, on_select: impl FnMut() + 'static) -> Self {
        Self {
            label: Some(label.into()),
            on_select: Some(Box::new(on_select)),
        }
    }

    /// Creates a non-interactive separator between groups of menu entries.
    pub fn separator() -> Self {
        Self {
            label: None,
            on_select: None,
        }
    }
}

/// A user-defined rendering pass hosted by a [`CustomGpuSurface`].
///
/// Implement this trait to draw arbitrary wgpu content inside the widget tree.
/// The surface owns a texture target, calls `render` whenever it needs a new
/// frame (on initial paint, after a resize, and every frame while its
/// animation signal is active), and composites the result as an image with the
/// configured corner radius. Implementations must be `'static` because the
/// surface boxes them.
pub trait CustomGpuPass: 'static {
    /// Records draw commands into `encoder` for `view`.
    ///
    /// `width` and `height` are the backing texture dimensions, already scaled
    /// by [`CustomGpuSurface::resolution_scale`], and `format` is the texture
    /// format that the returned render pass must target. `time_secs` is the
    /// elapsed animation time supplied by the window.
    #[allow(clippy::too_many_arguments)]
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        time_secs: f32,
    );
}

/// A node in the declarative widget tree.
///
/// Every widget builder converts into one of these variants via [`IntoElement`].
/// Containers store `Element` children, and [`WidgetArena::mount`] walks the
/// tree to create the corresponding live nodes. The enum is intentionally
/// finite: adding a new widget means adding a variant and a mount case.
pub enum Element {
    /// A general-purpose layout container; see [`Container`].
    Container(Container),
    /// A frosted/liquid panel backed by a container; see [`GlassPanel`].
    Glass(GlassPanel),
    /// A single- or multi-line text label; see [`Text`].
    Text(Text),
    /// A raster image loaded from a file, SVG data, or decoded pixels; see [`Image`].
    Image(Image),
    /// An SVG icon rasterized to a square texture; see [`Icon`].
    Icon(Icon),
    /// A clickable button; see [`Button`].
    Button(Button),
    /// A two-state toggle bound to a boolean signal; see [`Switch`].
    Switch(Switch),
    /// A continuous value control bound to a float signal; see [`Slider`].
    Slider(Slider),
    /// A read-only progress indicator bound to a float signal; see [`ProgressBar`].
    ProgressBar(ProgressBar),
    /// A single-line text editor bound to a string signal; see [`TextInput`].
    TextInput(TextInput),
    /// A multiline text editor bound to a string signal; see [`TextArea`].
    TextArea(TextArea),
    /// A vertically scrolling viewport with layout-managed content; see [`ScrollView`].
    ScrollView(ScrollView),
    /// Two resizable panes with a draggable divider; see [`SplitPane`].
    SplitPane(SplitPane),
    /// A windowed list that builds only the visible rows; see [`VirtualizedList`].
    VirtualizedList(VirtualizedList),
    /// A container that shows exactly one selected child; see [`Switcher`].
    Switcher(Switcher),
    /// A container whose child is chosen by a boolean signal; see [`Show`].
    Show(Show),
    /// A container reconciled from a signal-driven list of keyed items; see [`ForEach`].
    ForEach(ForEach),
    /// A segmented tab selector bound to an index signal; see [`Tabs`].
    Tabs(Tabs),
    /// A drop-down selector bound to an index signal; see [`Dropdown`].
    Dropdown(Dropdown),
    /// A widget that hosts a user-supplied wgpu render pass; see [`CustomGpuSurface`].
    CustomGpuSurface(CustomGpuSurface),
    /// The macOS traffic-light window controls; see [`TrafficLights`].
    TrafficLights(TrafficLights),
    /// A one-pixel horizontal rule with a configurable color; see [`Divider`].
    Divider(Divider),
    /// An empty node that absorbs remaining flex space; see [`Spacer`].
    Spacer(Spacer),
}

/// Conversion from a widget builder into a mountable [`Element`].
///
/// Implement this trait for custom builders that should be accepted anywhere
/// an `Element` is expected, such as [`Container::child`]. All built-in widget
/// types implement it, as does [`Element`] itself (the identity conversion).
pub trait IntoElement {
    /// Consumes `self` and produces the corresponding [`Element`] variant.
    fn into_element(self) -> Element;
}

impl IntoElement for Element {
    fn into_element(self) -> Element {
        self
    }
}

