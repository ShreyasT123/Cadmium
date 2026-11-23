use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slotmap::SlotMap;

use crate::animation::Spring;
use crate::core::{Color, CornerRadii, Rect};
use crate::debug::FrameProfiler;
use crate::events::{FocusManager, Modifiers, WidgetId};
use crate::layout::{
    AlignItems, Edges, FlexDir, JustifyContent, LayoutEngine, LayoutNode, LayoutStyle, Length,
    snap_rect_to_pixels,
};
use crate::platform::Clipboard;
use crate::reactive::{EffectId, Signal, create_effect, dispose_effect};
use crate::renderer::{GlassQuad, SdfQuad};
use crate::style::{MacTheme, Material, Shadow, ThemeMode};
use crate::text::{FontWeight, TextBlock, TextEngine, TextStyle};
use crate::window::{CursorStyle, FrameContext, WindowCommand, WindowDelegate};

const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(530);

#[derive(Debug, Clone, Copy, Default)]
pub struct DirtyFlags {
    pub state: bool,
    pub layout: bool,
    pub paint: bool,
}

type DirtyQueue = Rc<RefCell<Vec<(WidgetId, DirtyFlags)>>>;

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

fn lookup_offset_x(char_offsets: &[(usize, f32)], byte_idx: usize) -> f32 {
    for &(idx, x_pos) in char_offsets {
        if idx >= byte_idx {
            return x_pos;
        }
    }
    char_offsets.last().map(|&(_, x)| x).unwrap_or(0.0)
}

pub enum Element {
    Container(Container),
    Glass(GlassPanel),
    Text(Text),
    Button(Button),
    Switch(Switch),
    Slider(Slider),
    TextInput(TextInput),
    ScrollView(ScrollView),
    Tabs(Tabs),
    Dropdown(Dropdown),
    TrafficLights(TrafficLights),
    Divider(Divider),
    Spacer(Spacer),
}

pub trait IntoElement {
    fn into_element(self) -> Element;
}

impl IntoElement for Element {
    fn into_element(self) -> Element {
        self
    }
}

pub struct Container {
    layout: LayoutStyle,
    material: Material,
    radius: f32,
    radius_signal: Option<Signal<f32>>,
    is_drag_region: bool,
    children: Vec<Element>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            layout: LayoutStyle::default(),
            material: Material::solid(Color::TRANSPARENT),
            radius: 0.0,
            radius_signal: None,
            is_drag_region: false,
            children: Vec::new(),
        }
    }

    pub fn column() -> Self {
        Self::new().direction(FlexDir::Column)
    }

    pub fn row() -> Self {
        Self::new().direction(FlexDir::Row)
    }

    pub fn direction(mut self, dir: FlexDir) -> Self {
        self.layout.direction = dir;
        self
    }

    pub fn width(mut self, w: Length) -> Self {
        self.layout.width = w;
        self
    }

    pub fn height(mut self, h: Length) -> Self {
        self.layout.height = h;
        self
    }

    pub fn flex_grow(mut self, g: f32) -> Self {
        self.layout.flex_grow = g;
        self
    }

    pub fn padding(mut self, p: Edges) -> Self {
        self.layout.padding = p;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.layout.gap = gap;
        self
    }

    pub fn align(mut self, align: AlignItems) -> Self {
        self.layout.align_items = Some(align);
        self
    }

    pub fn justify(mut self, justify: JustifyContent) -> Self {
        self.layout.justify_content = Some(justify);
        self
    }

    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    pub fn fill(mut self, color: Color) -> Self {
        self.material = Material::solid(color);
        self
    }

    pub fn gradient(mut self, start: Color, end: Color) -> Self {
        self.material = Material::Classic {
            fill: start,
            fill_end: Some(end),
            gradient_dir: [0.0, 1.0],
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            shadow: None,
        };
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        match &mut self.material {
            Material::Classic {
                border_color,
                border_width,
                ..
            }
            | Material::Frosted {
                border_color,
                border_width,
                ..
            }
            | Material::Liquid {
                border_color,
                border_width,
                ..
            } => {
                *border_color = color;
                *border_width = width;
            }
        }
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    pub fn bind_radius(mut self, sig: Signal<f32>) -> Self {
        self.radius_signal = Some(sig);
        self
    }

    pub fn shadow(mut self, color: Color, blur: f32, offset_y: f32) -> Self {
        let shad = Some(Shadow::new(color, blur, offset_y));
        match &mut self.material {
            Material::Classic { shadow, .. }
            | Material::Frosted { shadow, .. }
            | Material::Liquid { shadow, .. } => {
                *shadow = shad;
            }
        }
        self
    }

    pub fn drag_region(mut self) -> Self {
        self.is_drag_region = true;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }

    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        self.children
            .extend(iter.into_iter().map(IntoElement::into_element));
        self
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Container {
    fn into_element(self) -> Element {
        Element::Container(self)
    }
}

pub type Column = Container;

pub struct Row;
impl Row {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::row()
    }
}

pub struct GlassPanel {
    inner: Container,
}

impl GlassPanel {
    pub fn new() -> Self {
        Self {
            inner: Container::new()
                .radius(16.0)
                .material(Material::frosted(Color::from_rgba8(255, 255, 255, 130))),
        }
    }

    pub fn width(mut self, w: Length) -> Self {
        self.inner = self.inner.width(w);
        self
    }

    pub fn height(mut self, h: Length) -> Self {
        self.inner = self.inner.height(h);
        self
    }

    pub fn flex_grow(mut self, g: f32) -> Self {
        self.inner = self.inner.flex_grow(g);
        self
    }

    pub fn padding(mut self, p: Edges) -> Self {
        self.inner = self.inner.padding(p);
        self
    }

    pub fn gap(mut self, g: f32) -> Self {
        self.inner = self.inner.gap(g);
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.inner = self.inner.radius(r);
        self
    }

    pub fn tint(mut self, tint: Color) -> Self {
        self.inner.material = Material::frosted(tint);
        self
    }

    pub fn liquid(mut self) -> Self {
        self.inner.material = Material::liquid(Color::from_rgba8(255, 255, 255, 40));
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        self.inner = self.inner.children(iter);
        self
    }
}

impl Default for GlassPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for GlassPanel {
    fn into_element(self) -> Element {
        Element::Container(self.inner)
    }
}

enum TextSource {
    Static(String),
    Reactive(Box<dyn FnMut() -> String>),
}

pub struct Text {
    source: TextSource,
    style: TextStyle,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            source: TextSource::Static(text.into()),
            style: TextStyle::default(),
        }
    }

    pub fn reactive(f: impl FnMut() -> String + 'static) -> Self {
        Self {
            source: TextSource::Reactive(Box::new(f)),
            style: TextStyle::default(),
        }
    }

    pub fn size(mut self, font_size: f32) -> Self {
        self.style.font_size = font_size;
        self.style.line_height = (font_size * 1.38).round();
        self
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.style.weight = weight;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self
    }
}

impl IntoElement for Text {
    fn into_element(self) -> Element {
        Element::Text(self)
    }
}

pub struct Button {
    label: String,
    accent: bool,
    on_click: Option<Box<dyn FnMut()>>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            accent: false,
            on_click: None,
        }
    }

    pub fn primary(mut self) -> Self {
        self.accent = true;
        self
    }

    pub fn on_click(mut self, f: impl FnMut() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl IntoElement for Button {
    fn into_element(self) -> Element {
        Element::Button(self)
    }
}

pub struct Switch {
    signal: Signal<bool>,
}

impl Switch {
    pub fn new(signal: Signal<bool>) -> Self {
        Self { signal }
    }
}

impl IntoElement for Switch {
    fn into_element(self) -> Element {
        Element::Switch(self)
    }
}

pub struct Slider {
    signal: Signal<f32>,
    min: f32,
    max: f32,
    width: f32,
}

impl Slider {
    pub fn new(signal: Signal<f32>, min: f32, max: f32) -> Self {
        Self {
            signal,
            min,
            max,
            width: 160.0,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

impl IntoElement for Slider {
    fn into_element(self) -> Element {
        Element::Slider(self)
    }
}

pub struct TextInput {
    signal: Signal<String>,
    placeholder: String,
    material: Material,
    width: Length,
}

impl TextInput {
    pub fn new(signal: Signal<String>) -> Self {
        Self {
            signal,
            placeholder: "Search or type...".into(),
            material: Material::solid(Color::from_rgba8(255, 255, 255, 240)),
            width: Length::Px(240.0),
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
}

impl IntoElement for TextInput {
    fn into_element(self) -> Element {
        Element::TextInput(self)
    }
}

pub struct ScrollView {
    children: Vec<Element>,
    gap: f32,
}

impl ScrollView {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            gap: 8.0,
        }
    }

    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }

    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        self.children
            .extend(iter.into_iter().map(IntoElement::into_element));
        self
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for ScrollView {
    fn into_element(self) -> Element {
        Element::ScrollView(self)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TrafficLights;

impl TrafficLights {
    pub fn new() -> Self {
        Self
    }
}

impl IntoElement for TrafficLights {
    fn into_element(self) -> Element {
        Element::TrafficLights(self)
    }
}

pub struct Tabs {
    signal: Signal<usize>,
    items: Vec<String>,
    material: Material,
    width: Length,
}

impl Tabs {
    pub fn new(signal: Signal<usize>, items: &[impl AsRef<str>]) -> Self {
        Self {
            signal,
            items: items.iter().map(|s| s.as_ref().to_string()).collect(),
            material: Material::Classic {
                fill: Color::from_rgba8(229, 229, 234, 255),
                fill_end: None,
                gradient_dir: [0.0, 1.0],
                border_color: Color::TRANSPARENT,
                border_width: 0.0,
                shadow: None,
            },
            width: Length::Auto,
        }
    }

    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
}

impl IntoElement for Tabs {
    fn into_element(self) -> Element {
        Element::Tabs(self)
    }
}

pub struct Dropdown {
    signal: Signal<usize>,
    items: Vec<String>,
    material: Material,
    width: Length,
}

impl Dropdown {
    pub fn new(signal: Signal<usize>, items: &[impl AsRef<str>]) -> Self {
        Self {
            signal,
            items: items.iter().map(|s| s.as_ref().to_string()).collect(),
            material: Material::Classic {
                fill: Color::from_rgba8(255, 255, 255, 220),
                fill_end: None,
                gradient_dir: [0.0, 1.0],
                border_color: Color::from_rgba8(0, 0, 0, 30),
                border_width: 1.0,
                shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 18), 4.0, 1.0)),
            },
            width: Length::Px(160.0),
        }
    }

    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
}

impl IntoElement for Dropdown {
    fn into_element(self) -> Element {
        Element::Dropdown(self)
    }
}

pub struct Divider {
    color: Color,
}

impl Divider {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl IntoElement for Divider {
    fn into_element(self) -> Element {
        Element::Divider(self)
    }
}

pub struct Spacer {
    grow: f32,
}

impl Spacer {
    pub fn flex() -> Self {
        Self { grow: 1.0 }
    }
}

impl IntoElement for Spacer {
    fn into_element(self) -> Element {
        Element::Spacer(self)
    }
}

enum WidgetPayload {
    Container {
        material: Material,
        radius: Rc<Cell<f32>>,
        is_drag_region: bool,
    },
    Text {
        block: TextBlock,
        pending: Option<Rc<RefCell<String>>>,
    },
    Button {
        label: TextBlock,
        accent: bool,
        on_click: Option<Box<dyn FnMut()>>,
    },
    Switch {
        signal: Signal<bool>,
    },
    Slider {
        signal: Signal<f32>,
        min: f32,
        max: f32,
    },
    TextInput {
        signal: Signal<String>,
        material: Material,
        cursor: usize,
        selection: Option<(usize, usize)>,
        is_selecting: bool,
        text_block: TextBlock,
        placeholder_block: TextBlock,
        char_offsets: Vec<(usize, f32)>,
        last_synced_text: String,
    },
    ScrollView {
        scroll_y: Spring,
        content_height: f32,
        content_id: WidgetId,
    },
    Tabs {
        signal: Signal<usize>,
        items: Vec<String>,
        labels: Vec<TextBlock>,
        material: Material,
        pill_spring: Spring,
    },
    Dropdown {
        signal: Signal<usize>,
        items: Vec<String>,
        labels: Vec<TextBlock>,
        selected_labels: Vec<TextBlock>,
        material: Material,
        is_open: bool,
        chevron: TextBlock,
    },
    TrafficLights {
        hovered: bool,
    },
    Divider {
        color: Color,
    },
    Spacer,
}

struct WidgetNode {
    children: Vec<WidgetId>,
    layout_node: LayoutNode,
    layout_style: LayoutStyle,
    bounds: Rect,
    owned_effects: Vec<EffectId>,
    payload: WidgetPayload,
}

pub struct WidgetArena {
    nodes: SlotMap<WidgetId, WidgetNode>,
    layout: LayoutEngine,
    layout_to_widget: HashMap<LayoutNode, WidgetId>,
    root: Option<WidgetId>,
    focus: FocusManager,
    dirty_queue: DirtyQueue,
    scroll_ids: Vec<WidgetId>,
    tab_ids: Vec<WidgetId>,
    traffic_ids: Vec<WidgetId>,
    needs_layout: bool,
}

impl WidgetArena {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            layout: LayoutEngine::new(),
            layout_to_widget: HashMap::new(),
            root: None,
            focus: FocusManager::new(),
            dirty_queue: Rc::new(RefCell::new(Vec::new())),
            scroll_ids: Vec::new(),
            tab_ids: Vec::new(),
            traffic_ids: Vec::new(),
            needs_layout: true,
        }
    }

    pub fn mount(&mut self, element: Element, text: &mut TextEngine) -> WidgetId {
        self.clear();
        let root_id = self.mount_node(element, text);
        self.root = Some(root_id);
        self.needs_layout = true;
        root_id
    }

    pub fn clear(&mut self) {
        for (_, node) in self.nodes.drain() {
            for eff in node.owned_effects {
                dispose_effect(eff);
            }
            self.layout.remove(node.layout_node);
        }
        self.layout_to_widget.clear();
        self.dirty_queue.borrow_mut().clear();
        self.scroll_ids.clear();
        self.tab_ids.clear();
        self.traffic_ids.clear();
        self.focus = FocusManager::new();
        self.root = None;
    }

    fn mount_node(&mut self, element: Element, text: &mut TextEngine) -> WidgetId {
        match element {
            Element::Container(c) => {
                let child_ids: Vec<WidgetId> = c
                    .children
                    .into_iter()
                    .map(|ch| self.mount_node(ch, text))
                    .collect();
                let child_layout_nodes: Vec<LayoutNode> = child_ids
                    .iter()
                    .map(|id| self.nodes[*id].layout_node)
                    .collect();
                let lnode = self.layout.new_container(c.layout, &child_layout_nodes);
                let radius_cell = Rc::new(Cell::new(c.radius));

                let id = self.nodes.insert(WidgetNode {
                    children: child_ids,
                    layout_node: lnode,
                    layout_style: c.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Container {
                        material: c.material,
                        radius: radius_cell.clone(),
                        is_drag_region: c.is_drag_region,
                    },
                });
                self.layout_to_widget.insert(lnode, id);

                if let Some(sig) = c.radius_signal {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        radius_cell.set(sig.get());
                        push_dirty(
                            &q,
                            id,
                            DirtyFlags {
                                state: false,
                                layout: false,
                                paint: true,
                            },
                        );
                    });
                    self.nodes[id].owned_effects.push(eff);
                }
                id
            }
            Element::Glass(g) => self.mount_node(Element::Container(g.inner), text),
            Element::Text(t) => {
                let lstyle = LayoutStyle::default();
                let lnode = self.layout.new_leaf(lstyle);
                let shared = Rc::new(RefCell::new(String::new()));
                let is_reactive = matches!(t.source, TextSource::Reactive(_));
                let initial = match &t.source {
                    TextSource::Static(s) => s.clone(),
                    TextSource::Reactive(_) => String::new(),
                };
                let block = TextBlock::new(text, &initial, t.style);

                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Text {
                        block,
                        pending: if is_reactive {
                            Some(shared.clone())
                        } else {
                            None
                        },
                    },
                });
                self.layout_to_widget.insert(lnode, id);

                if let TextSource::Reactive(mut f) = t.source {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        let next = f();
                        let mut slot = shared.borrow_mut();
                        if *slot != next {
                            *slot = next;
                            push_dirty(
                                &q,
                                id,
                                DirtyFlags {
                                    state: true,
                                    layout: true,
                                    paint: true,
                                },
                            );
                        }
                    });
                    self.nodes[id].owned_effects.push(eff);
                }
                id
            }
            Element::Button(b) => {
                let label_block = TextBlock::new(
                    text,
                    &b.label,
                    TextStyle {
                        font_size: 13.0,
                        line_height: 18.0,
                        color: if b.accent {
                            Color::from_rgba8(255, 255, 255, 255)
                        } else {
                            Color::from_rgba8(29, 29, 31, 255)
                        },
                        weight: FontWeight::Medium,
                    },
                );
                let lstyle = LayoutStyle {
                    height: Length::Px(30.0),
                    padding: Edges::xy(14.0, 6.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Button {
                        label: label_block,
                        accent: b.accent,
                        on_click: b.on_click,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::Switch(sw) => {
                let lstyle = LayoutStyle {
                    width: Length::Px(38.0),
                    height: Length::Px(22.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Switch { signal: sw.signal },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = sw.signal.get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: false,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::Slider(sl) => {
                let lstyle = LayoutStyle {
                    width: Length::Px(sl.width),
                    height: Length::Px(24.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Slider {
                        signal: sl.signal,
                        min: sl.min,
                        max: sl.max,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = sl.signal.get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: false,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::TextInput(ti) => {
                let lstyle = LayoutStyle {
                    width: ti.width,
                    height: Length::Px(34.0),
                    padding: Edges::xy(12.0, 7.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let current_val = ti.signal.get();
                let cursor = current_val.len();
                let text_block = TextBlock::new(
                    text,
                    &current_val,
                    TextStyle {
                        font_size: 13.5,
                        line_height: 18.0,
                        color: Color::from_rgba8(28, 28, 32, 255),
                        weight: FontWeight::Regular,
                    },
                );
                let mut char_offsets = Vec::new();
                text_block.collect_char_offsets(&mut char_offsets);

                let placeholder_block = TextBlock::new(
                    text,
                    &ti.placeholder,
                    TextStyle {
                        font_size: 13.5,
                        line_height: 18.0,
                        color: Color::from_rgba8(142, 142, 147, 255),
                        weight: FontWeight::Regular,
                    },
                );

                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::TextInput {
                        signal: ti.signal,
                        material: ti.material,
                        cursor,
                        selection: None,
                        is_selecting: false,
                        text_block,
                        placeholder_block,
                        char_offsets,
                        last_synced_text: current_val,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = ti.signal.get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: true,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::Tabs(tb) => {
                let count = tb.items.len().max(1);
                let lstyle = LayoutStyle {
                    width: tb.width,
                    min_width: Length::Px(count as f32 * 68.0),
                    height: Length::Px(30.0),
                    padding: Edges::all(2.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);

                let labels = tb
                    .items
                    .iter()
                    .map(|it| {
                        TextBlock::new(
                            text,
                            it,
                            TextStyle {
                                font_size: 12.5,
                                line_height: 16.0,
                                color: Color::from_rgba8(28, 28, 32, 255),
                                weight: FontWeight::Medium,
                            },
                        )
                    })
                    .collect();

                let initial_tab = tb.signal.get();
                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Tabs {
                        signal: tb.signal,
                        items: tb.items,
                        labels,
                        material: tb.material,
                        pill_spring: Spring::new(initial_tab as f32, 320.0, 26.0),
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = tb.signal.get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: false,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                self.tab_ids.push(id);
                id
            }
            Element::Dropdown(dd) => {
                let lstyle = LayoutStyle {
                    width: dd.width,
                    height: Length::Px(32.0),
                    padding: Edges::xy(12.0, 6.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let labels = dd
                    .items
                    .iter()
                    .map(|it| {
                        TextBlock::new(
                            text,
                            it,
                            TextStyle {
                                font_size: 13.0,
                                line_height: 18.0,
                                color: Color::from_rgba8(28, 28, 32, 255),
                                weight: FontWeight::Medium,
                            },
                        )
                    })
                    .collect();
                let selected_labels = dd
                    .items
                    .iter()
                    .map(|it| {
                        TextBlock::new(
                            text,
                            it,
                            TextStyle {
                                font_size: 13.0,
                                line_height: 18.0,
                                color: Color::from_rgba8(255, 255, 255, 255),
                                weight: FontWeight::Semibold,
                            },
                        )
                    })
                    .collect();
                let chevron = TextBlock::new(
                    text,
                    "⌵",
                    TextStyle {
                        font_size: 11.0,
                        line_height: 14.0,
                        color: Color::from_rgba8(120, 120, 128, 255),
                        weight: FontWeight::Bold,
                    },
                );

                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Dropdown {
                        signal: dd.signal,
                        items: dd.items,
                        labels,
                        selected_labels,
                        material: dd.material,
                        is_open: false,
                        chevron,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = dd.signal.get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: false,
                            layout: false,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.layout_to_widget.insert(lnode, id);
                self.focus.focus_order.push(id);
                id
            }
            Element::TrafficLights(_) => {
                let lstyle = LayoutStyle {
                    width: Length::Px(56.0),
                    height: Length::Px(16.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::TrafficLights { hovered: false },
                });
                self.layout_to_widget.insert(lnode, id);
                self.traffic_ids.push(id);
                id
            }
            Element::ScrollView(sv) => {
                let child_ids: Vec<WidgetId> = sv
                    .children
                    .into_iter()
                    .map(|ch| self.mount_node(ch, text))
                    .collect();
                let child_lnodes: Vec<LayoutNode> = child_ids
                    .iter()
                    .map(|id| self.nodes[*id].layout_node)
                    .collect();

                let content_lstyle = LayoutStyle {
                    direction: FlexDir::Column,
                    gap: sv.gap,
                    flex_shrink: 0.0,
                    ..Default::default()
                };
                let content_lnode = self.layout.new_container(content_lstyle, &child_lnodes);
                let content_id = self.nodes.insert(WidgetNode {
                    children: child_ids,
                    layout_node: content_lnode,
                    layout_style: content_lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Container {
                        material: Material::solid(Color::TRANSPARENT),
                        radius: Rc::new(Cell::new(0.0)),
                        is_drag_region: false,
                    },
                });
                self.layout_to_widget.insert(content_lnode, content_id);

                let viewport_lstyle = LayoutStyle {
                    direction: FlexDir::Column,
                    flex_grow: 1.0,
                    ..Default::default()
                };
                let viewport_lnode = self.layout.new_container(viewport_lstyle, &[content_lnode]);

                let id = self.nodes.insert(WidgetNode {
                    children: vec![content_id],
                    layout_node: viewport_lnode,
                    layout_style: viewport_lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::ScrollView {
                        scroll_y: Spring::new(0.0, 260.0, 24.0),
                        content_height: 0.0,
                        content_id,
                    },
                });
                self.layout_to_widget.insert(viewport_lnode, id);
                self.scroll_ids.push(id);
                id
            }
            Element::Divider(d) => {
                let lstyle = LayoutStyle {
                    height: Length::Px(1.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Divider { color: d.color },
                });
                self.layout_to_widget.insert(lnode, id);
                id
            }
            Element::Spacer(s) => {
                let lstyle = LayoutStyle {
                    flex_grow: s.grow,
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    payload: WidgetPayload::Spacer,
                });
                self.layout_to_widget.insert(lnode, id);
                id
            }
        }
    }
}

impl Default for WidgetArena {
    fn default() -> Self {
        Self::new()
    }
}

struct OverlayRenderRecord {
    source_id: WidgetId,
    trigger_bounds: Rect,
    selected: usize,
    item_count: usize,
}

struct PaintCtx<'a, 'b> {
    ctx: &'a mut FrameContext<'b>,
    theme: &'a MacTheme,
    scale: f32,
    caret_visible: bool,
    hovered: Option<WidgetId>,
    focused: Option<WidgetId>,
    overlays: &'a mut Vec<OverlayRenderRecord>,
}

pub struct WidgetWindow<F> {
    build_fn: F,
    theme_signal: Option<Signal<ThemeMode>>,
    arena: WidgetArena,
    profiler: FrameProfiler,
    last_size: (f32, f32),
    hovered: Option<WidgetId>,
    captured: Option<WidgetId>,
    open_dropdowns: Vec<(WidgetId, Rect, usize)>,
    caret_visible: bool,
    next_blink_at: Option<Instant>,
}

impl<F> WidgetWindow<F>
where
    F: FnMut() -> Element + 'static,
{
    pub fn new(build_fn: F) -> Self {
        Self {
            build_fn,
            theme_signal: None,
            arena: WidgetArena::new(),
            profiler: FrameProfiler::default(),
            last_size: (0.0, 0.0),
            hovered: None,
            captured: None,
            open_dropdowns: Vec::new(),
            caret_visible: true,
            next_blink_at: None,
        }
    }

    pub fn with_theme(mut self, theme_signal: Signal<ThemeMode>) -> Self {
        self.theme_signal = Some(theme_signal);
        self
    }

    pub fn profiler(&self) -> &FrameProfiler {
        &self.profiler
    }

    fn hit_test(&self, x: f32, y: f32) -> Option<WidgetId> {
        self.arena
            .root
            .and_then(|root| hit_test_node(root, &self.arena.nodes, x, y, (0.0, 0.0)))
    }

    fn update_slider(&mut self, id: WidgetId, x: f32) -> bool {
        if let Some(node) = self.arena.nodes.get(id)
            && let WidgetPayload::Slider { signal, min, max } = node.payload
        {
            let t = ((x - node.bounds.x) / node.bounds.width.max(1.0)).clamp(0.0, 1.0);
            let val = min + (max - min) * t;
            signal.set(val);
            return true;
        }
        false
    }

    fn reset_caret_blink(&mut self) {
        let is_text_focused = self
            .arena
            .focus
            .focused
            .and_then(|id| self.arena.nodes.get(id))
            .is_some_and(|n| matches!(n.payload, WidgetPayload::TextInput { .. }));

        if is_text_focused {
            self.caret_visible = true;
            self.next_blink_at = Some(Instant::now() + CARET_BLINK_INTERVAL);
        } else {
            self.caret_visible = false;
            self.next_blink_at = None;
        }
    }
}

impl<F> WindowDelegate for WidgetWindow<F>
where
    F: FnMut() -> Element + 'static,
{
    fn init(&mut self, text: &mut TextEngine) {
        self.hovered = None;
        self.captured = None;
        self.open_dropdowns.clear();
        self.caret_visible = false;
        self.next_blink_at = None;

        let root_elem = (self.build_fn)();
        self.arena.mount(root_elem, text);
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        for &sid in &self.arena.scroll_ids {
            if let Some(node) = self.arena.nodes.get_mut(sid)
                && let WidgetPayload::ScrollView { scroll_y, .. } = &mut node.payload
            {
                animating |= scroll_y.step(dt);
            }
        }
        for &tid in &self.arena.tab_ids {
            if let Some(node) = self.arena.nodes.get_mut(tid)
                && let WidgetPayload::Tabs {
                    signal,
                    pill_spring,
                    ..
                } = &mut node.payload
            {
                pill_spring.set_target(signal.get() as f32);
                animating |= pill_spring.step(dt);
            }
        }

        if let Some(blink_at) = self.next_blink_at {
            let now = Instant::now();
            if now >= blink_at {
                self.caret_visible = !self.caret_visible;
                self.next_blink_at = Some(now + CARET_BLINK_INTERVAL);
            }
        }

        // Only return true for active spring motion; caret blink uses WaitUntil via next_redraw_at()
        animating
    }

    fn next_redraw_at(&self) -> Option<Instant> {
        self.next_blink_at
    }

    fn cursor_style(&self) -> CursorStyle {
        if let Some(h) = self.hovered {
            if let Some(node) = self.arena.nodes.get(h)
                && matches!(node.payload, WidgetPayload::TextInput { .. })
            {
                return CursorStyle::Default;
            }
            CursorStyle::Pointer
        } else if self.captured.is_some() {
            CursorStyle::Pointer
        } else {
            CursorStyle::Default
        }
    }

    fn is_drag_region(&self, x: f32, y: f32) -> bool {
        if self.hit_test(x, y).is_some() {
            return false;
        }
        self.arena
            .root
            .map(|root| is_drag_node(root, &self.arena.nodes, x, y, (0.0, 0.0)))
            .unwrap_or(false)
    }

    fn on_scroll(&mut self, _delta_x: f32, delta_y: f32) -> bool {
        let mut handled = false;
        for &sid in &self.arena.scroll_ids {
            if let Some(node) = self.arena.nodes.get_mut(sid)
                && let WidgetPayload::ScrollView {
                    scroll_y,
                    content_height,
                    ..
                } = &mut node.payload
            {
                let max_scroll = (*content_height - node.bounds.height).max(0.0);
                if max_scroll > 0.0 {
                    let next = (scroll_y.target - delta_y).clamp(0.0, max_scroll);
                    scroll_y.set_target(next);
                    handled = true;
                }
            }
        }
        handled
    }

    fn on_key(
        &mut self,
        key: &winit::keyboard::Key,
        text: Option<&str>,
        modifiers: &Modifiers,
    ) -> bool {
        let Some(focused_id) = self.arena.focus.focused else {
            return false;
        };

        let Some(node) = self.arena.nodes.get_mut(focused_id) else {
            return false;
        };

        let mut handled = false;
        if let WidgetPayload::TextInput {
            signal,
            cursor,
            selection,
            ..
        } = &mut node.payload
        {
            let mut val = signal.get();
            *cursor = snap_char_boundary(&val, *cursor);
            *selection = normalize_selection(&val, *selection);

            let is_ctrl = modifiers.ctrl || modifiers.super_key;
            let is_shortcut_char = |ch: &str| -> bool {
                matches!(key, winit::keyboard::Key::Character(c) if c.eq_ignore_ascii_case(ch))
            };

            let delete_sel =
                |val: &mut String, cursor: &mut usize, sel: &mut Option<(usize, usize)>| -> bool {
                    if let Some((start, end)) = sel.take() {
                        let s = snap_char_boundary(val, start.min(end));
                        let e = snap_char_boundary(val, start.max(end));
                        if s < e {
                            val.drain(s..e);
                        }
                        *cursor = s;
                        return true;
                    }
                    false
                };

            if is_ctrl && is_shortcut_char("a") {
                *selection = Some((0, val.len()));
                *cursor = val.len();
                handled = true;
            } else if is_ctrl && is_shortcut_char("c") {
                if let Some((start, end)) = *selection {
                    let s = snap_char_boundary(&val, start.min(end));
                    let e = snap_char_boundary(&val, start.max(end));
                    let _ = Clipboard::set_text(&val[s..e]);
                    handled = true;
                }
            } else if is_ctrl && is_shortcut_char("x") {
                if let Some((start, end)) = *selection {
                    let s = snap_char_boundary(&val, start.min(end));
                    let e = snap_char_boundary(&val, start.max(end));
                    let _ = Clipboard::set_text(&val[s..e]);
                    delete_sel(&mut val, cursor, selection);
                    signal.set(val);
                    handled = true;
                }
            } else if is_ctrl && is_shortcut_char("v") {
                if let Ok(pasted) = Clipboard::get_text() {
                    let clean = pasted.replace('\r', "").replace('\n', " ");
                    delete_sel(&mut val, cursor, selection);
                    val.insert_str(*cursor, &clean);
                    *cursor += clean.len();
                    signal.set(val);
                    handled = true;
                }
            } else {
                match key {
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => {
                        if delete_sel(&mut val, cursor, selection) {
                            signal.set(val);
                            handled = true;
                        } else if *cursor > 0 && !val.is_empty() {
                            let prev = val[..*cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            val.drain(prev..*cursor);
                            *cursor = prev;
                            signal.set(val);
                            handled = true;
                        }
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Delete) => {
                        if delete_sel(&mut val, cursor, selection) {
                            signal.set(val);
                            handled = true;
                        } else if *cursor < val.len() {
                            let next = val[*cursor..]
                                .char_indices()
                                .nth(1)
                                .map(|(i, _)| *cursor + i)
                                .unwrap_or(val.len());
                            val.drain(*cursor..next);
                            signal.set(val);
                            handled = true;
                        }
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowLeft) => {
                        *selection = None;
                        if *cursor > 0 {
                            *cursor = val[..*cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            handled = true;
                        }
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight) => {
                        *selection = None;
                        if *cursor < val.len() {
                            *cursor = val[*cursor..]
                                .char_indices()
                                .nth(1)
                                .map(|(i, _)| *cursor + i)
                                .unwrap_or(val.len());
                            handled = true;
                        }
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Home) => {
                        *selection = None;
                        *cursor = 0;
                        handled = true;
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::End) => {
                        *selection = None;
                        *cursor = val.len();
                        handled = true;
                    }
                    _ => {
                        if !is_ctrl
                            && let Some(ch_str) = text
                            && !ch_str.chars().any(|c| c.is_control())
                        {
                            delete_sel(&mut val, cursor, selection);
                            val.insert_str(*cursor, ch_str);
                            *cursor += ch_str.len();
                            signal.set(val);
                            handled = true;
                        }
                    }
                }
            }
        }

        if handled {
            self.reset_caret_blink();
        }
        handled
    }

    fn on_pointer_move(&mut self, x: f32, y: f32) -> bool {
        if let Some(cap) = self.captured
            && let Some(node) = self.arena.nodes.get_mut(cap)
            && let WidgetPayload::TextInput {
                signal,
                cursor,
                selection,
                is_selecting,
                char_offsets,
                ..
            } = &mut node.payload
            && *is_selecting
        {
            let val = signal.get();
            let rel_x = x - (node.bounds.x + 12.0);
            let anchor = snap_char_boundary(&val, selection.map(|s| s.0).unwrap_or(*cursor));
            let new_cursor = snap_char_boundary(&val, find_char_index_at_x(char_offsets, rel_x));
            *cursor = new_cursor;
            *selection = if anchor != new_cursor {
                Some((anchor, new_cursor))
            } else {
                None
            };
            return true;
        }
        if let Some(cap) = self.captured {
            return self.update_slider(cap, x);
        }

        let next = self.hit_test(x, y);
        let mut redraw = false;

        for &tid in &self.arena.traffic_ids {
            if let Some(node) = self.arena.nodes.get_mut(tid)
                && let WidgetPayload::TrafficLights { hovered } = &mut node.payload
            {
                let inside = node.bounds.contains(x, y);
                if *hovered != inside {
                    *hovered = inside;
                    redraw = true;
                }
            }
        }

        if next != self.hovered {
            self.hovered = next;
            redraw = true;
        }
        redraw
    }

    fn on_pointer_down(&mut self, x: f32, y: f32) -> (bool, WindowCommand) {
        if !self.open_dropdowns.is_empty() {
            let overlays = std::mem::take(&mut self.open_dropdowns);
            for (dd_id, menu_rect, count) in overlays.iter().rev().copied() {
                if menu_rect.contains(x, y) {
                    let row_h = 30.0;
                    let clicked_idx = ((y - (menu_rect.y + 4.0)) / row_h).floor().max(0.0) as usize;
                    let idx = clicked_idx.min(count.saturating_sub(1));
                    if let Some(node) = self.arena.nodes.get_mut(dd_id)
                        && let WidgetPayload::Dropdown {
                            signal, is_open, ..
                        } = &mut node.payload
                    {
                        signal.set(idx);
                        *is_open = false;
                    }
                    return (true, WindowCommand::None);
                }
            }
            // Clicked outside: close all open dropdowns
            for (dd_id, _, _) in overlays {
                if let Some(node) = self.arena.nodes.get_mut(dd_id)
                    && let WidgetPayload::Dropdown { is_open, .. } = &mut node.payload
                {
                    *is_open = false;
                }
            }
        }

        let Some(hit) = self.hit_test(x, y) else {
            let focus_changed = self.arena.focus.set_focus(None);
            self.reset_caret_blink();
            if self.is_drag_region(x, y) {
                return (false, WindowCommand::None);
            }
            return (focus_changed, WindowCommand::None);
        };

        self.arena.focus.set_focus(Some(hit));
        let mut clicked = false;
        let mut capture_target = false;

        if let Some(node) = self.arena.nodes.get_mut(hit) {
            match &mut node.payload {
                WidgetPayload::TrafficLights { .. } => {
                    let rel_x = x - node.bounds.x;
                    let cmd = if rel_x <= 18.0 {
                        WindowCommand::Close
                    } else if rel_x <= 38.0 {
                        WindowCommand::Minimize
                    } else {
                        WindowCommand::ToggleMaximize
                    };
                    return (true, cmd);
                }
                WidgetPayload::Button { on_click, .. } => {
                    if let Some(cb) = on_click {
                        cb();
                    }
                    clicked = true;
                }
                WidgetPayload::Switch { signal } => {
                    signal.update(|v| *v = !*v);
                    clicked = true;
                }
                WidgetPayload::Slider { .. } => {
                    capture_target = true;
                    clicked = true;
                }
                WidgetPayload::Tabs { signal, items, .. } => {
                    let tab_w = node.bounds.width / items.len().max(1) as f32;
                    let idx = ((x - node.bounds.x) / tab_w).floor().max(0.0) as usize;
                    signal.set(idx.min(items.len().saturating_sub(1)));
                    clicked = true;
                }
                WidgetPayload::Dropdown { is_open, .. } => {
                    *is_open = !*is_open;
                    clicked = true;
                }
                WidgetPayload::TextInput {
                    signal,
                    cursor,
                    selection,
                    is_selecting,
                    char_offsets,
                    ..
                } => {
                    let val = signal.get();
                    let rel_x = x - (node.bounds.x + 12.0);
                    let char_idx =
                        snap_char_boundary(&val, find_char_index_at_x(char_offsets, rel_x));
                    *cursor = char_idx;
                    *selection = None;
                    *is_selecting = true;
                    capture_target = true;
                    clicked = true;
                }
                _ => {}
            }
        }

        self.reset_caret_blink();

        if capture_target {
            self.captured = Some(hit);
            self.update_slider(hit, x);
        }
        (clicked, WindowCommand::None)
    }

    fn on_pointer_up(&mut self) -> bool {
        let mut was = self.captured.is_some();
        if let Some(cap) = self.captured.take()
            && let Some(node) = self.arena.nodes.get_mut(cap)
            && let WidgetPayload::TextInput { is_selecting, .. } = &mut node.payload
        {
            *is_selecting = false;
            was = true;
        }
        was
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) {
        let frame_start = FrameProfiler::begin_timer();

        let mode = self
            .theme_signal
            .map(|s| s.get())
            .unwrap_or(ThemeMode::Light);
        let theme = MacTheme::from_mode(mode);

        // Stage 1: Drain per-arena reactive dirty notifications
        let dirty_batch: Vec<(WidgetId, DirtyFlags)> =
            std::mem::take(&mut *self.arena.dirty_queue.borrow_mut());
        for (id, flags) in dirty_batch {
            if let Some(node) = self.arena.nodes.get_mut(id) {
                if flags.layout {
                    self.arena.layout.mark_dirty(node.layout_node);
                    self.arena.needs_layout = true;
                }
                match &mut node.payload {
                    WidgetPayload::Text {
                        block,
                        pending: Some(cell),
                    } => {
                        let text_val = cell.borrow().clone();
                        block.set_text(ctx.text_engine(), &text_val);
                    }
                    WidgetPayload::TextInput {
                        signal,
                        cursor,
                        selection,
                        text_block,
                        char_offsets,
                        last_synced_text,
                        ..
                    } => {
                        let text_val = signal.get();
                        if *last_synced_text != text_val {
                            *last_synced_text = text_val.clone();
                            *cursor = snap_char_boundary(&text_val, *cursor);
                            *selection = normalize_selection(&text_val, *selection);
                            text_block.set_text(ctx.text_engine(), &text_val);
                            text_block.collect_char_offsets(char_offsets);
                        }
                    }
                    _ => {}
                }
            }
        }

        if (ctx.width, ctx.height) != self.last_size {
            self.last_size = (ctx.width, ctx.height);
            self.arena.needs_layout = true;
        }

        // Stage 2: Incremental Taffy Layout + ScrollView content_height calculation
        let layout_start = FrameProfiler::begin_timer();
        let mut did_layout = false;
        if self.arena.needs_layout
            && let Some(root) = self.arena.root
        {
            let root_lnode = self.arena.nodes[root].layout_node;
            let mut rstyle = self.arena.nodes[root].layout_style;
            rstyle.width = Length::Px(ctx.width);
            rstyle.height = Length::Px(ctx.height);
            self.arena.layout.set_style(root_lnode, rstyle);

            let nodes = &mut self.arena.nodes;
            let lmap = &self.arena.layout_to_widget;
            let text_engine = ctx.text_engine();

            self.arena.layout.compute_with_measure(
                root_lnode,
                self.last_size.0,
                self.last_size.1,
                |lnode, _, _, avail_w, _| {
                    let Some(&wid) = lmap.get(&lnode) else {
                        return (0.0, 0.0);
                    };
                    let Some(node) = nodes.get_mut(wid) else {
                        return (0.0, 0.0);
                    };
                    match &mut node.payload {
                        WidgetPayload::Text { block, .. } => block.measure(text_engine, avail_w),
                        WidgetPayload::Button { label, .. } => {
                            let (w, _) = label.measure(text_engine, None);
                            (w + 28.0, 30.0)
                        }
                        _ => (0.0, 0.0),
                    }
                },
            );

            let rects = self.arena.layout.collect_logical_rects(root_lnode);
            for (lnode, rect) in rects {
                if let Some(&wid) = self.arena.layout_to_widget.get(&lnode)
                    && let Some(node) = self.arena.nodes.get_mut(wid)
                {
                    node.bounds = rect;
                }
            }

            for &sid in &self.arena.scroll_ids {
                let cid = match self.arena.nodes.get(sid).map(|n| &n.payload) {
                    Some(WidgetPayload::ScrollView { content_id, .. }) => *content_id,
                    _ => continue,
                };
                let ch = self
                    .arena
                    .nodes
                    .get(cid)
                    .map(|n| n.bounds.height)
                    .unwrap_or(0.0);
                if let Some(snode) = self.arena.nodes.get_mut(sid)
                    && let WidgetPayload::ScrollView { content_height, .. } = &mut snode.payload
                {
                    *content_height = ch;
                }
            }

            self.arena.needs_layout = false;
            did_layout = true;
        }
        let layout_ms = layout_start.elapsed().as_secs_f32() * 1000.0;

        // Stage 3: Hierarchical Paint + Clamped Overlay Portals
        let paint_start = FrameProfiler::begin_timer();
        let mut overlay_records = Vec::new();
        self.open_dropdowns.clear();

        if let Some(root) = self.arena.root {
            let scale = ctx.scale;
            let mut pctx = PaintCtx {
                ctx,
                theme: &theme,
                scale,
                caret_visible: self.caret_visible,
                hovered: self.hovered,
                focused: self.arena.focus.focused,
                overlays: &mut overlay_records,
            };

            paint_node(root, &self.arena.nodes, &mut pctx, (0.0, 0.0));

            for ov in overlay_records {
                let row_h = 30.0;
                let menu_w = ov.trigger_bounds.width.max(160.0).min(ctx.width - 16.0);
                let menu_h = ov.item_count as f32 * row_h + 8.0;
                let menu_x = ov
                    .trigger_bounds
                    .x
                    .clamp(8.0, (ctx.width - menu_w - 8.0).max(8.0));

                let space_below =
                    ctx.height - (ov.trigger_bounds.y + ov.trigger_bounds.height + 4.0);
                let menu_y = if space_below >= menu_h + 8.0 {
                    ov.trigger_bounds.y + ov.trigger_bounds.height + 4.0
                } else {
                    (ov.trigger_bounds.y - menu_h - 4.0).max(8.0)
                };
                let menu_rect = Rect::new(menu_x, menu_y, menu_w, menu_h);
                self.open_dropdowns
                    .push((ov.source_id, menu_rect, ov.item_count));

                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(menu_rect, scale),
                    radii: CornerRadii::all(10.0).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: Color::from_rgba8(0, 0, 0, 75),
                    shadow_offset: [0.0, 8.0 * scale],
                    shadow_blur: 20.0 * scale,
                    ..Default::default()
                });
                ctx.push_glass(GlassQuad {
                    rect: snap_rect_to_pixels(menu_rect, scale),
                    radii: CornerRadii::all(10.0).scale(scale),
                    tint: Color::from_rgba8(250, 250, 255, 185),
                    border_color: theme.hairline,
                    border_width: 1.0,
                    frost: 0.40,
                    specular: 0.85,
                    ..Default::default()
                });

                if let Some(dd_node) = self.arena.nodes.get(ov.source_id)
                    && let WidgetPayload::Dropdown {
                        labels,
                        selected_labels,
                        ..
                    } = &dd_node.payload
                {
                    for i in 0..ov.item_count {
                        let ry = menu_y + 4.0 + i as f32 * row_h;
                        let row_rect = Rect::new(menu_x + 4.0, ry, menu_w - 8.0, row_h);
                        let is_sel = i == ov.selected;
                        if is_sel {
                            ctx.push_sdf(SdfQuad {
                                rect: snap_rect_to_pixels(row_rect, scale),
                                radii: CornerRadii::all(6.0).scale(scale),
                                fill: theme.accent,
                                ..Default::default()
                            });
                        }
                        let block = if is_sel {
                            &selected_labels[i]
                        } else {
                            &labels[i]
                        };
                        ctx.push_text(block, row_rect.x + 10.0, row_rect.y + 6.0);
                    }
                }
            }
        }

        let paint_ms = paint_start.elapsed().as_secs_f32() * 1000.0;
        let frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.profiler
            .record_frame(frame_ms, layout_ms, paint_ms, did_layout);
    }
}

fn paint_node(
    wid: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    pctx: &mut PaintCtx<'_, '_>,
    offset: (f32, f32),
) {
    let Some(node) = nodes.get(wid) else {
        return;
    };

    let bounds = Rect::new(
        node.bounds.x + offset.0,
        node.bounds.y + offset.1,
        node.bounds.width,
        node.bounds.height,
    );

    let scale = pctx.scale;
    let theme = pctx.theme;
    let is_hovered = pctx.hovered == Some(wid);
    let is_focused = pctx.focused == Some(wid);

    match &node.payload {
        WidgetPayload::Container {
            material, radius, ..
        } => {
            render_material(pctx.ctx, bounds, *material, radius.get(), scale);
            for &child in &node.children {
                paint_node(child, nodes, pctx, offset);
            }
        }
        WidgetPayload::Text { block, .. } => {
            pctx.ctx.push_text(block, bounds.x, bounds.y);
        }
        WidgetPayload::Button { label, accent, .. } => {
            let fill = if *accent {
                theme.accent
            } else if is_hovered {
                theme.control_hover
            } else {
                theme.panel_bg
            };
            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(7.0).scale(scale),
                fill,
                border_color: if *accent {
                    Color::TRANSPARENT
                } else {
                    theme.hairline
                },
                border_width: if *accent { 0.0 } else { 1.0 },
                shadow_color: theme.shadow_soft,
                shadow_offset: [0.0, 1.0 * scale],
                shadow_blur: 3.0 * scale,
                ..Default::default()
            });
            pctx.ctx.push_text(label, bounds.x + 14.0, bounds.y + 6.0);
        }
        WidgetPayload::Switch { signal } => {
            let on = signal.get();
            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(11.0).scale(scale),
                fill: if on {
                    theme.switch_on
                } else {
                    theme.switch_off
                },
                ..Default::default()
            });
            let tx = if on { bounds.x + 18.0 } else { bounds.x + 2.0 };
            pctx.ctx.push_sdf(SdfQuad {
                rect: Rect::new(tx, bounds.y + 2.0, 18.0, 18.0).scale(scale),
                radii: CornerRadii::all(9.0).scale(scale),
                fill: theme.control_thumb,
                shadow_color: theme.shadow_soft,
                shadow_offset: [0.0, 1.0 * scale],
                shadow_blur: 3.0 * scale,
                ..Default::default()
            });
        }
        WidgetPayload::Slider { signal, min, max } => {
            let val = signal.get();
            let frac = ((val - *min) / (*max - *min).max(0.0001)).clamp(0.0, 1.0);
            let track = Rect::new(
                bounds.x,
                bounds.y + (bounds.height - 4.0) * 0.5,
                bounds.width,
                4.0,
            );
            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(track, scale),
                radii: CornerRadii::all(2.0).scale(scale),
                fill: theme.control_track,
                ..Default::default()
            });
            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(
                    Rect::new(track.x, track.y, track.width * frac, 4.0),
                    scale,
                ),
                radii: CornerRadii::all(2.0).scale(scale),
                fill: theme.accent,
                ..Default::default()
            });
            let kx = track.x + track.width * frac - 8.0;
            let ky = bounds.y + (bounds.height - 16.0) * 0.5;
            pctx.ctx.push_sdf(SdfQuad {
                rect: Rect::new(kx, ky, 16.0, 16.0).scale(scale),
                radii: CornerRadii::all(8.0).scale(scale),
                fill: theme.control_thumb,
                border_color: theme.hairline,
                border_width: 1.0,
                shadow_color: theme.shadow_elevated,
                shadow_offset: [0.0, 1.5 * scale],
                shadow_blur: 4.0 * scale,
                ..Default::default()
            });
        }
        WidgetPayload::TextInput {
            signal,
            material,
            cursor,
            selection,
            text_block,
            placeholder_block,
            char_offsets,
            ..
        } => {
            let val = signal.get();
            render_material(pctx.ctx, bounds, *material, 8.0, scale);

            if is_focused {
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(8.0).scale(scale),
                    fill: Color::TRANSPARENT,
                    border_color: theme.accent,
                    border_width: 2.0 * scale,
                    ..Default::default()
                });
            }

            let text_x = bounds.x + 12.0;
            let text_y = bounds.y + 8.0;

            if let Some((start, end)) = *selection {
                let s = snap_char_boundary(&val, start.min(end));
                let e = snap_char_boundary(&val, start.max(end));
                if s != e {
                    let x1 = lookup_offset_x(char_offsets, s);
                    let x2 = lookup_offset_x(char_offsets, e);
                    let sel_rect = Rect::new(text_x + x1, text_y - 1.0, (x2 - x1).max(2.0), 18.0);
                    pctx.ctx.push_sdf(SdfQuad {
                        rect: snap_rect_to_pixels(sel_rect, scale),
                        radii: CornerRadii::all(3.0).scale(scale),
                        fill: Color::rgba(theme.accent.r, theme.accent.g, theme.accent.b, 0.28),
                        ..Default::default()
                    });
                }
            }

            if val.is_empty() {
                pctx.ctx.push_text(placeholder_block, text_x, text_y);
            } else {
                pctx.ctx.push_text(text_block, text_x, text_y);
            }

            if is_focused && pctx.caret_visible {
                let safe_cursor = snap_char_boundary(&val, *cursor);
                let caret_offset_x = lookup_offset_x(char_offsets, safe_cursor);
                let caret_x = text_x + caret_offset_x;
                let caret_rect = Rect::new(caret_x, text_y + 1.0, 1.5, 16.0);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(caret_rect, scale),
                    fill: theme.accent,
                    ..Default::default()
                });
            }
        }
        WidgetPayload::Tabs {
            items,
            labels,
            material,
            pill_spring,
            ..
        } => {
            render_material(pctx.ctx, bounds, *material, 7.0, scale);

            let count = items.len().max(1);
            let tab_w = (bounds.width - 4.0) / count as f32;
            let pill_x = bounds.x + 2.0 + pill_spring.value * tab_w;
            let pill_rect = Rect::new(pill_x, bounds.y + 2.0, tab_w, bounds.height - 4.0);

            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(pill_rect, scale),
                radii: CornerRadii::all(5.5).scale(scale),
                fill: theme.control_thumb,
                border_color: theme.hairline,
                border_width: 0.5 * scale,
                shadow_color: theme.shadow_soft,
                shadow_offset: [0.0, 1.0 * scale],
                shadow_blur: 3.0 * scale,
                ..Default::default()
            });

            for (i, label) in labels.iter().enumerate() {
                let item_cx = bounds.x + 2.0 + (i as f32 + 0.5) * tab_w;
                let lw = label.style().font_size * 0.55 * items[i].len() as f32;
                pctx.ctx
                    .push_text(label, item_cx - lw * 0.5, bounds.y + 7.0);
            }
        }
        WidgetPayload::Dropdown {
            signal,
            items,
            labels,
            material,
            is_open,
            chevron,
            ..
        } => {
            render_material(pctx.ctx, bounds, *material, 8.0, scale);
            let selected = signal.get().min(items.len().saturating_sub(1));
            if let Some(lbl) = labels.get(selected) {
                pctx.ctx.push_text(lbl, bounds.x + 12.0, bounds.y + 7.0);
            }
            pctx.ctx
                .push_text(chevron, bounds.x + bounds.width - 20.0, bounds.y + 7.0);

            if *is_open {
                pctx.overlays.push(OverlayRenderRecord {
                    source_id: wid,
                    trigger_bounds: bounds,
                    selected,
                    item_count: items.len(),
                });
            }
        }
        WidgetPayload::ScrollView {
            scroll_y,
            content_height,
            content_id,
        } => {
            let viewport = bounds;
            pctx.ctx.push_clip(viewport);

            let child_offset = (offset.0, offset.1 - scroll_y.value);
            paint_node(*content_id, nodes, pctx, child_offset);

            pctx.ctx.pop_clip();

            if *content_height > viewport.height {
                let track_h = viewport.height - 8.0;
                let thumb_h = ((viewport.height / *content_height) * track_h).clamp(24.0, track_h);
                let max_scroll = (*content_height - viewport.height).max(1.0);
                let thumb_t = (scroll_y.value / max_scroll).clamp(0.0, 1.0);
                let thumb_y = viewport.y + 4.0 + thumb_t * (track_h - thumb_h);
                let thumb_rect =
                    Rect::new(viewport.x + viewport.width - 6.0, thumb_y, 4.0, thumb_h);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(thumb_rect, scale),
                    radii: CornerRadii::all(2.0).scale(scale),
                    fill: Color::from_rgba8(0, 0, 0, 75),
                    ..Default::default()
                });
            }
        }
        WidgetPayload::TrafficLights { hovered } => {
            let cy = bounds.y + (bounds.height - 12.0) * 0.5;
            let r1 = snap_rect_to_pixels(Rect::new(bounds.x, cy, 12.0, 12.0), scale);
            let r2 = snap_rect_to_pixels(Rect::new(bounds.x + 20.0, cy, 12.0, 12.0), scale);
            let r3 = snap_rect_to_pixels(Rect::new(bounds.x + 40.0, cy, 12.0, 12.0), scale);

            for (rect, fill) in [
                (r1, theme.traffic_close),
                (r2, theme.traffic_minimize),
                (r3, theme.traffic_zoom),
            ] {
                pctx.ctx.push_sdf(SdfQuad {
                    rect,
                    radii: CornerRadii::all(rect.width * 0.5),
                    fill,
                    border_color: theme.traffic_border,
                    border_width: 1.0,
                    ..Default::default()
                });
            }

            if *hovered {
                let bar_len = 6.2 * scale;
                let bar_thick = 1.35 * scale;
                let bar_r = CornerRadii::all(bar_thick * 0.5);

                let cx1 = r1.x + r1.width * 0.5;
                let cy1 = r1.y + r1.height * 0.5;
                let x_len = 6.6 * scale;
                let red_glyph = Color::from_rgba8(77, 10, 8, 225);
                for rot in [std::f32::consts::FRAC_PI_4, -std::f32::consts::FRAC_PI_4] {
                    pctx.ctx.push_sdf(SdfQuad {
                        rect: Rect::new(cx1 - x_len * 0.5, cy1 - bar_thick * 0.5, x_len, bar_thick),
                        radii: bar_r,
                        fill: red_glyph,
                        rotation: rot,
                        ..Default::default()
                    });
                }

                let cx2 = r2.x + r2.width * 0.5;
                let cy2 = r2.y + r2.height * 0.5;
                pctx.ctx.push_sdf(SdfQuad {
                    rect: Rect::new(
                        cx2 - bar_len * 0.5,
                        cy2 - bar_thick * 0.5,
                        bar_len,
                        bar_thick,
                    ),
                    radii: bar_r,
                    fill: Color::from_rgba8(153, 87, 0, 225),
                    ..Default::default()
                });

                let cx3 = r3.x + r3.width * 0.5;
                let cy3 = r3.y + r3.height * 0.5;
                let green_glyph = Color::from_rgba8(0, 101, 0, 225);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: Rect::new(
                        cx3 - bar_len * 0.5,
                        cy3 - bar_thick * 0.5,
                        bar_len,
                        bar_thick,
                    ),
                    radii: bar_r,
                    fill: green_glyph,
                    ..Default::default()
                });
                pctx.ctx.push_sdf(SdfQuad {
                    rect: Rect::new(
                        cx3 - bar_thick * 0.5,
                        cy3 - bar_len * 0.5,
                        bar_thick,
                        bar_len,
                    ),
                    radii: bar_r,
                    fill: green_glyph,
                    ..Default::default()
                });
            }
        }
        WidgetPayload::Divider { color } => {
            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                fill: *color,
                ..Default::default()
            });
        }
        WidgetPayload::Spacer => {}
    }
}

fn hit_test_node(
    wid: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    x: f32,
    y: f32,
    offset: (f32, f32),
) -> Option<WidgetId> {
    let node = nodes.get(wid)?;
    let bounds = Rect::new(
        node.bounds.x + offset.0,
        node.bounds.y + offset.1,
        node.bounds.width,
        node.bounds.height,
    );

    if let WidgetPayload::ScrollView {
        scroll_y,
        content_id,
        ..
    } = &node.payload
    {
        if !bounds.contains(x, y) {
            return None;
        }
        let child_offset = (offset.0, offset.1 - scroll_y.value);
        return hit_test_node(*content_id, nodes, x, y, child_offset);
    }

    for &child in node.children.iter().rev() {
        if let Some(hit) = hit_test_node(child, nodes, x, y, offset) {
            return Some(hit);
        }
    }

    if bounds.contains(x, y)
        && matches!(
            node.payload,
            WidgetPayload::Button { .. }
                | WidgetPayload::Switch { .. }
                | WidgetPayload::Slider { .. }
                | WidgetPayload::TextInput { .. }
                | WidgetPayload::Tabs { .. }
                | WidgetPayload::Dropdown { .. }
                | WidgetPayload::TrafficLights { .. }
        )
    {
        return Some(wid);
    }

    None
}

fn is_drag_node(
    wid: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    x: f32,
    y: f32,
    offset: (f32, f32),
) -> bool {
    let Some(node) = nodes.get(wid) else {
        return false;
    };
    let bounds = Rect::new(
        node.bounds.x + offset.0,
        node.bounds.y + offset.1,
        node.bounds.width,
        node.bounds.height,
    );
    if let WidgetPayload::Container {
        is_drag_region: true,
        ..
    } = &node.payload
        && bounds.contains(x, y)
    {
        return true;
    }
    for &child in &node.children {
        if is_drag_node(child, nodes, x, y, offset) {
            return true;
        }
    }
    false
}

fn render_material(
    ctx: &mut FrameContext<'_>,
    bounds: Rect,
    material: Material,
    radius: f32,
    scale: f32,
) {
    match material {
        Material::Classic {
            fill,
            fill_end,
            gradient_dir,
            border_color,
            border_width,
            shadow,
        } => {
            let (shad_col, shad_blur, shad_off) = match shadow {
                Some(s) => (
                    s.color,
                    s.blur * scale,
                    [s.offset[0] * scale, s.offset[1] * scale],
                ),
                None => (Color::TRANSPARENT, 0.0, [0.0, 0.0]),
            };
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(radius).scale(scale),
                fill,
                fill_end,
                gradient_dir,
                rotation: 0.0,
                border_color,
                border_width: border_width * scale,
                shadow_color: shad_col,
                shadow_offset: shad_off,
                shadow_blur: shad_blur,
            });
        }
        Material::Frosted {
            tint,
            frost,
            specular,
            border_color,
            border_width,
            shadow,
        } => {
            if let Some(s) = shadow {
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(radius).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: s.color,
                    shadow_offset: [s.offset[0] * scale, s.offset[1] * scale],
                    shadow_blur: s.blur * scale,
                    ..Default::default()
                });
            }
            ctx.push_glass(GlassQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(radius).scale(scale),
                tint,
                border_color,
                border_width,
                saturation: 1.35,
                specular,
                frost,
                liquid_ripple: 0.0,
                ..Default::default()
            });
        }
        Material::Liquid {
            tint,
            frost,
            ripple,
            specular,
            rim_angle,
            sheen_progress,
            conic_strength,
            border_color,
            border_width,
            shadow,
        } => {
            if let Some(s) = shadow {
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(radius).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: s.color,
                    shadow_offset: [s.offset[0] * scale, s.offset[1] * scale],
                    shadow_blur: s.blur * scale,
                    ..Default::default()
                });
            }
            ctx.push_glass(GlassQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(radius).scale(scale),
                tint,
                border_color,
                border_width,
                saturation: 1.25,
                specular,
                frost,
                liquid_ripple: ripple,
                rim_angle,
                sheen_progress,
                conic_strength,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactive::create_signal;

    #[test]
    fn utf8_boundary_snapping_prevents_mid_char_panics() {
        let s = "Café🦀";
        // 'é' spans bytes 3..5; '🦀' spans bytes 5..9
        assert_eq!(snap_char_boundary(s, 4), 3);
        assert_eq!(snap_char_boundary(s, 7), 5);
        assert_eq!(snap_char_boundary(s, 99), s.len());
    }

    #[test]
    fn dirty_queues_are_isolated_per_arena() {
        let mut te = TextEngine::new();
        let sig_a = create_signal("A".to_string());
        let sig_b = create_signal("B".to_string());

        let mut arena_a = WidgetArena::new();
        let mut arena_b = WidgetArena::new();
        arena_a.mount(Text::reactive(move || sig_a.get()).into_element(), &mut te);
        arena_b.mount(Text::reactive(move || sig_b.get()).into_element(), &mut te);

        arena_a.dirty_queue.borrow_mut().clear();
        arena_b.dirty_queue.borrow_mut().clear();

        sig_a.set("A2".to_string());
        crate::reactive::flush_effects();

        assert_eq!(arena_a.dirty_queue.borrow().len(), 1);
        assert_eq!(arena_b.dirty_queue.borrow().len(), 0);

        sig_a.dispose();
        sig_b.dispose();
    }
}
