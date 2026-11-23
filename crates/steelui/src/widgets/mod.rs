use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use slotmap::SlotMap;

use crate::accessibility::{build_tree_update, A11yNodeRecord, A11yRole};
use crate::animation::Spring;
use crate::core::{Color, CornerRadii, Rect};
use crate::debug::FrameProfiler;
use crate::events::{FocusManager, Modifiers, WidgetId};
use crate::layout::{
    snap_rect_to_pixels, AlignItems, Edges, FlexDir, JustifyContent, LayoutEngine, LayoutNode,
    LayoutStyle, Length,
};
use crate::platform::Clipboard;
use crate::reactive::{create_effect, dispose_effect, EffectId, Signal};
use crate::renderer::{GlassQuad, SdfQuad};
use crate::style::{MacTheme, Material, Shadow, ThemeMode};
use crate::text::{FontWeight, TextBlock, TextEngine, TextStyle};
use crate::window::{CursorStyle, FrameContext, WindowCommand, WindowDelegate};

#[derive(Debug, Clone, Copy, Default)]
pub struct DirtyFlags {
    pub state: bool,
    pub layout: bool,
    pub paint: bool,
}

thread_local! {
    static PENDING_WIDGET_DIRTY: RefCell<Vec<(WidgetId, DirtyFlags)>> = const { RefCell::new(Vec::new()) };
}

fn push_widget_dirty(id: WidgetId, flags: DirtyFlags) {
    PENDING_WIDGET_DIRTY.with(|q| q.borrow_mut().push((id, flags)));
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
            Material::Classic { border_color, border_width, .. }
            | Material::Frosted { border_color, border_width, .. }
            | Material::Liquid { border_color, border_width, .. } => {
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
        placeholder: String,
        material: Material,
        cursor: usize,
        text_block: TextBlock,
        placeholder_block: TextBlock,
    },
    ScrollView {
        scroll_y: Spring,
        content_height: f32,
        content_id: WidgetId,
    },
    TrafficLights {
        hovered: bool,
        x_glyph: TextBlock,
        min_glyph: TextBlock,
        plus_glyph: TextBlock,
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
    a11y_role: A11yRole,
    a11y_label: Option<String>,
    payload: WidgetPayload,
}

pub struct WidgetArena {
    nodes: SlotMap<WidgetId, WidgetNode>,
    layout: LayoutEngine,
    layout_to_widget: HashMap<LayoutNode, WidgetId>,
    root: Option<WidgetId>,
    focus: FocusManager,
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
                    a11y_role: A11yRole::Container,
                    a11y_label: None,
                    payload: WidgetPayload::Container {
                        material: c.material,
                        radius: radius_cell.clone(),
                        is_drag_region: c.is_drag_region,
                    },
                });
                self.layout_to_widget.insert(lnode, id);

                if let Some(sig) = c.radius_signal {
                    let cell = radius_cell;
                    let eff = create_effect(move || {
                        cell.set(sig.get());
                        push_widget_dirty(
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
                    a11y_role: A11yRole::Label,
                    a11y_label: if is_reactive { None } else { Some(initial) },
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
                    let eff = create_effect(move || {
                        let next = f();
                        let mut slot = shared.borrow_mut();
                        if *slot != next {
                            *slot = next;
                            push_widget_dirty(
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
                    a11y_role: A11yRole::Button,
                    a11y_label: Some(b.label),
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
                    a11y_role: A11yRole::Switch,
                    a11y_label: Some("Toggle".into()),
                    payload: WidgetPayload::Switch { signal: sw.signal },
                });
                let eff = create_effect(move || {
                    let _ = sw.signal.get();
                    push_widget_dirty(
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
                    a11y_role: A11yRole::Slider,
                    a11y_label: Some("Slider".into()),
                    payload: WidgetPayload::Slider {
                        signal: sl.signal,
                        min: sl.min,
                        max: sl.max,
                    },
                });
                let eff = create_effect(move || {
                    let _ = sl.signal.get();
                    push_widget_dirty(
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
                    a11y_role: A11yRole::Container,
                    a11y_label: Some(ti.placeholder.clone()),
                    payload: WidgetPayload::TextInput {
                        signal: ti.signal,
                        placeholder: ti.placeholder,
                        material: ti.material,
                        cursor,
                        text_block,
                        placeholder_block,
                    },
                });
                let eff = create_effect(move || {
                    let _ = ti.signal.get();
                    push_widget_dirty(
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
                let gs = TextStyle {
                    font_size: 9.5,
                    line_height: 11.0,
                    color: Color::from_rgba8(77, 10, 8, 210),
                    weight: FontWeight::Bold,
                };
                let x_glyph = TextBlock::new(text, "×", gs);
                let min_glyph = TextBlock::new(text, "−", gs);
                let plus_glyph = TextBlock::new(text, "+", gs);

                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    a11y_role: A11yRole::Container,
                    a11y_label: Some("Window Controls".into()),
                    payload: WidgetPayload::TrafficLights {
                        hovered: false,
                        x_glyph,
                        min_glyph,
                        plus_glyph,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
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
                    a11y_role: A11yRole::Container,
                    a11y_label: None,
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
                let viewport_lnode =
                    self.layout.new_container(viewport_lstyle, &[content_lnode]);

                let id = self.nodes.insert(WidgetNode {
                    children: vec![content_id],
                    layout_node: viewport_lnode,
                    layout_style: viewport_lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    a11y_role: A11yRole::Container,
                    a11y_label: None,
                    payload: WidgetPayload::ScrollView {
                        scroll_y: Spring::new(0.0, 260.0, 24.0),
                        content_height: 0.0,
                        content_id,
                    },
                });
                self.layout_to_widget.insert(viewport_lnode, id);
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
                    a11y_role: A11yRole::Container,
                    a11y_label: None,
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
                    a11y_role: A11yRole::Container,
                    a11y_label: None,
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

pub struct WidgetWindow<F> {
    build_fn: F,
    theme_signal: Option<Signal<ThemeMode>>,
    arena: WidgetArena,
    profiler: FrameProfiler,
    last_size: (f32, f32),
    hovered: Option<WidgetId>,
    captured: Option<WidgetId>,
    start_instant: Instant,
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
            start_instant: Instant::now(),
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
        if let Some(node) = self.arena.nodes.get(id) {
            if let WidgetPayload::Slider { signal, min, max } = node.payload {
                let t = ((x - node.bounds.x) / node.bounds.width.max(1.0)).clamp(0.0, 1.0);
                let val = min + (max - min) * t;
                signal.set(val);
                return true;
            }
        }
        false
    }
}

impl<F> WindowDelegate for WidgetWindow<F>
where
    F: FnMut() -> Element + 'static,
{
    fn init(&mut self, text: &mut TextEngine) {
        let root_elem = (self.build_fn)();
        self.arena.mount(root_elem, text);
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        for node in self.arena.nodes.values_mut() {
            if let WidgetPayload::ScrollView { scroll_y, .. } = &mut node.payload {
                animating |= scroll_y.step(dt);
            }
        }
        if self.arena.focus.focused.is_some() {
            animating = true;
        }
        animating
    }

    fn cursor_style(&self) -> CursorStyle {
        if let Some(h) = self.hovered {
            if let Some(node) = self.arena.nodes.get(h) {
                if matches!(node.payload, WidgetPayload::TextInput { .. }) {
                    return CursorStyle::Default;
                }
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
        for node in self.arena.nodes.values_mut() {
            if let WidgetPayload::ScrollView {
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

        if let WidgetPayload::TextInput { signal, cursor, .. } = &mut node.payload {
            let mut val = signal.get();
            *cursor = (*cursor).min(val.len());

            match key {
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => {
                    if *cursor > 0 && !val.is_empty() {
                        let prev = val[..*cursor]
                            .char_indices()
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        val.drain(prev..*cursor);
                        *cursor = prev;
                        signal.set(val);
                        return true;
                    }
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Delete) => {
                    if *cursor < val.len() {
                        let next = val[*cursor..]
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| *cursor + i)
                            .unwrap_or(val.len());
                        val.drain(*cursor..next);
                        signal.set(val);
                        return true;
                    }
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowLeft) => {
                    if *cursor > 0 {
                        *cursor = val[..*cursor]
                            .char_indices()
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        return true;
                    }
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight) => {
                    if *cursor < val.len() {
                        *cursor = val[*cursor..]
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| *cursor + i)
                            .unwrap_or(val.len());
                        return true;
                    }
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Home) => {
                    *cursor = 0;
                    return true;
                }
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::End) => {
                    *cursor = val.len();
                    return true;
                }
                _ => {
                    if (modifiers.ctrl || modifiers.super_key) && text == Some("v") {
                        if let Ok(pasted) = Clipboard::get_text() {
                            let clean = pasted.replace('\r', "").replace('\n', " ");
                            val.insert_str(*cursor, &clean);
                            *cursor += clean.len();
                            signal.set(val);
                            return true;
                        }
                    }

                    if let Some(ch_str) = text {
                        if !ch_str.chars().any(|c| c.is_control()) {
                            val.insert_str(*cursor, ch_str);
                            *cursor += ch_str.len();
                            signal.set(val);
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn on_pointer_move(&mut self, x: f32, y: f32) -> bool {
        if let Some(cap) = self.captured {
            return self.update_slider(cap, x);
        }
        let next = self.hit_test(x, y);
        let mut redraw = false;

        for node in self.arena.nodes.values_mut() {
            if let WidgetPayload::TrafficLights { hovered, .. } = &mut node.payload {
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
        let Some(hit) = self.hit_test(x, y) else {
            self.arena.focus.set_focus(None);
            return (true, WindowCommand::None);
        };
        self.arena.focus.set_focus(Some(hit));
        let mut clicked = false;
        let mut capture_slider = false;

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
                    capture_slider = true;
                    clicked = true;
                }
                WidgetPayload::TextInput { signal, cursor, .. } => {
                    let val = signal.get();
                    *cursor = val.len();
                    clicked = true;
                }
                _ => {}
            }
        }

        if capture_slider {
            self.captured = Some(hit);
            self.update_slider(hit, x);
        }
        (clicked, WindowCommand::None)
    }

    fn on_pointer_up(&mut self) -> bool {
        let was = self.captured.is_some();
        self.captured = None;
        was
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) {
        let frame_start = FrameProfiler::begin_timer();

        let mode = self
            .theme_signal
            .map(|s| s.get())
            .unwrap_or(ThemeMode::Light);
        let theme = MacTheme::from_mode(mode);

        // Stage 1: Drain reactive widget dirty notifications
        let dirty_batch: Vec<(WidgetId, DirtyFlags)> =
            PENDING_WIDGET_DIRTY.with(|q| std::mem::take(&mut *q.borrow_mut()));
        for (id, flags) in dirty_batch {
            if let Some(node) = self.arena.nodes.get_mut(id) {
                if flags.layout {
                    self.arena.layout.mark_dirty(node.layout_node);
                    self.arena.needs_layout = true;
                }
                match &mut node.payload {
                    WidgetPayload::Text { block, pending } => {
                        if let Some(cell) = pending {
                            let text_val = cell.borrow().clone();
                            block.set_text(ctx.text_engine(), &text_val);
                            node.a11y_label = Some(text_val);
                        }
                    }
                    WidgetPayload::TextInput { signal, text_block, .. } => {
                        let text_val = signal.get();
                        text_block.set_text(ctx.text_engine(), &text_val);
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
        if self.arena.needs_layout {
            if let Some(root) = self.arena.root {
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
                    if let Some(&wid) = self.arena.layout_to_widget.get(&lnode) {
                        if let Some(node) = self.arena.nodes.get_mut(wid) {
                            node.bounds = rect;
                        }
                    }
                }

                // Update content_height cleanly inside Stage 2 layout
                let scroll_updates: Vec<(WidgetId, f32)> = self.arena.nodes.iter()
                    .filter_map(|(id, node)| {
                        if let WidgetPayload::ScrollView { content_id, .. } = &node.payload {
                            self.arena.nodes.get(*content_id).map(|cn| (id, cn.bounds.height))
                        } else {
                            None
                        }
                    })
                    .collect();

                for (sid, ch) in scroll_updates {
                    if let Some(snode) = self.arena.nodes.get_mut(sid) {
                        if let WidgetPayload::ScrollView { content_height, .. } = &mut snode.payload {
                            *content_height = ch;
                        }
                    }
                }

                self.arena.needs_layout = false;
                did_layout = true;
            }
        }
        let layout_ms = layout_start.elapsed().as_secs_f32() * 1000.0;

        // Stage 3: Hierarchical Paint with Scissor Clipping & AccessKit semantic snapshot
        let paint_start = FrameProfiler::begin_timer();
        let mut a11y_records = Vec::new();
        if let Some(root) = self.arena.root {
            let scale = ctx.scale;
            let time_secs = self.start_instant.elapsed().as_secs_f32();

            paint_node(
                root,
                &self.arena.nodes,
                ctx,
                &theme,
                scale,
                time_secs,
                self.hovered,
                self.arena.focus.focused,
                (0.0, 0.0),
                &mut a11y_records,
            );

            let _tree_update = build_tree_update(root, self.arena.focus.focused, &a11y_records);
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
    ctx: &mut FrameContext<'_>,
    theme: &MacTheme,
    scale: f32,
    time_secs: f32,
    hovered: Option<WidgetId>,
    focused: Option<WidgetId>,
    offset: (f32, f32),
    a11y_records: &mut Vec<A11yNodeRecord>,
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

    let is_hovered = hovered == Some(wid);
    let is_focused = focused == Some(wid);

    a11y_records.push(A11yNodeRecord {
        id: wid,
        role: node.a11y_role,
        label: node.a11y_label.clone(),
        bounds,
        children: node.children.clone(),
    });

    match &node.payload {
        WidgetPayload::Container { material, radius, .. } => {
            render_material(ctx, bounds, *material, radius.get(), scale, theme);
            for &child in &node.children {
                paint_node(child, nodes, ctx, theme, scale, time_secs, hovered, focused, offset, a11y_records);
            }
        }
        WidgetPayload::Text { block, .. } => {
            ctx.push_text(block, bounds.x, bounds.y);
        }
        WidgetPayload::Button { label, accent, .. } => {
            let fill = if *accent {
                theme.accent
            } else if is_hovered {
                theme.control_hover
            } else {
                theme.panel_bg
            };
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(7.0).scale(scale),
                fill,
                border_color: if *accent { Color::TRANSPARENT } else { theme.hairline },
                border_width: if *accent { 0.0 } else { 1.0 },
                shadow_color: theme.shadow_soft,
                shadow_offset: [0.0, 1.0 * scale],
                shadow_blur: 3.0 * scale,
                ..Default::default()
            });
            ctx.push_text(label, bounds.x + 14.0, bounds.y + 6.0);
        }
        WidgetPayload::Switch { signal } => {
            let on = signal.get();
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(11.0).scale(scale),
                fill: if on { theme.switch_on } else { theme.switch_off },
                ..Default::default()
            });
            let tx = if on { bounds.x + 18.0 } else { bounds.x + 2.0 };
            ctx.push_sdf(SdfQuad {
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
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(track, scale),
                radii: CornerRadii::all(2.0).scale(scale),
                fill: theme.control_track,
                ..Default::default()
            });
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(Rect::new(track.x, track.y, track.width * frac, 4.0), scale),
                radii: CornerRadii::all(2.0).scale(scale),
                fill: theme.accent,
                ..Default::default()
            });
            let kx = track.x + track.width * frac - 8.0;
            let ky = bounds.y + (bounds.height - 16.0) * 0.5;
            ctx.push_sdf(SdfQuad {
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
        WidgetPayload::TextInput { signal, material, cursor, text_block, placeholder_block, .. } => {
            let val = signal.get();
            render_material(ctx, bounds, *material, 8.0, scale, theme);

            if is_focused {
                ctx.push_sdf(SdfQuad {
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

            if val.is_empty() {
                ctx.push_text(placeholder_block, text_x, text_y);
            } else {
                ctx.push_text(text_block, text_x, text_y);
            }

            if is_focused && (time_secs * 1.88).fract() < 0.52 {
                let prefix = &val[..(*cursor).min(val.len())];
                let caret_offset_x = if prefix.is_empty() {
                    0.0
                } else {
                    let mut temp = TextBlock::new(
                        ctx.text_engine(),
                        prefix,
                        TextStyle {
                            font_size: 13.5,
                            line_height: 18.0,
                            color: Color::TRANSPARENT,
                            weight: FontWeight::Regular,
                        },
                    );
                    temp.measure(ctx.text_engine(), None).0
                };

                let caret_x = text_x + caret_offset_x;
                let caret_rect = Rect::new(caret_x, text_y + 1.0, 1.5, 16.0);
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(caret_rect, scale),
                    fill: theme.accent,
                    ..Default::default()
                });
            }
        }
        WidgetPayload::ScrollView { scroll_y, content_height, content_id } => {
            let viewport = bounds;
            ctx.push_clip(viewport);

            let child_offset = (offset.0, offset.1 - scroll_y.value);
            paint_node(*content_id, nodes, ctx, theme, scale, time_secs, hovered, focused, child_offset, a11y_records);

            ctx.pop_clip();

            if *content_height > viewport.height {
                let track_h = viewport.height - 8.0;
                let thumb_h = ((viewport.height / *content_height) * track_h).clamp(24.0, track_h);
                let max_scroll = (*content_height - viewport.height).max(1.0);
                let thumb_t = (scroll_y.value / max_scroll).clamp(0.0, 1.0);
                let thumb_y = viewport.y + 4.0 + thumb_t * (track_h - thumb_h);
                let thumb_rect = Rect::new(viewport.x + viewport.width - 6.0, thumb_y, 4.0, thumb_h);
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(thumb_rect, scale),
                    radii: CornerRadii::all(2.0).scale(scale),
                    fill: Color::from_rgba8(0, 0, 0, 75),
                    ..Default::default()
                });
            }
        }
        WidgetPayload::TrafficLights {
            hovered,
            x_glyph,
            min_glyph,
            plus_glyph,
        } => {
            let cy = bounds.y + (bounds.height - 12.0) * 0.5;

            let r1 = Rect::new(bounds.x, cy, 12.0, 12.0);
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(r1, scale),
                radii: CornerRadii::all(6.0).scale(scale),
                fill: theme.traffic_close,
                border_color: theme.traffic_border,
                border_width: 1.0,
                ..Default::default()
            });

            let r2 = Rect::new(bounds.x + 20.0, cy, 12.0, 12.0);
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(r2, scale),
                radii: CornerRadii::all(6.0).scale(scale),
                fill: theme.traffic_minimize,
                border_color: theme.traffic_border,
                border_width: 1.0,
                ..Default::default()
            });

            let r3 = Rect::new(bounds.x + 40.0, cy, 12.0, 12.0);
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(r3, scale),
                radii: CornerRadii::all(6.0).scale(scale),
                fill: theme.traffic_zoom,
                border_color: theme.traffic_border,
                border_width: 1.0,
                ..Default::default()
            });

            if *hovered {
                ctx.push_text(x_glyph, r1.x + 2.8, r1.y + 0.5);
                ctx.push_text(min_glyph, r2.x + 2.8, r2.y + 0.5);
                ctx.push_text(plus_glyph, r3.x + 2.8, r3.y + 0.5);
            }
        }
        WidgetPayload::Divider { color } => {
            ctx.push_sdf(SdfQuad {
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
    let Some(node) = nodes.get(wid) else {
        return None;
    };
    let bounds = Rect::new(
        node.bounds.x + offset.0,
        node.bounds.y + offset.1,
        node.bounds.width,
        node.bounds.height,
    );

    if let WidgetPayload::ScrollView { scroll_y, content_id, .. } = &node.payload {
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
    if let WidgetPayload::Container { is_drag_region: true, .. } = &node.payload {
        if bounds.contains(x, y) {
            return true;
        }
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
    theme: &MacTheme,
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
                Some(s) => (s.color, s.blur * scale, [s.offset[0] * scale, s.offset[1] * scale]),
                None => (Color::TRANSPARENT, 0.0, [0.0, 0.0]),
            };
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(radius).scale(scale),
                fill,
                fill_end,
                gradient_dir,
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
