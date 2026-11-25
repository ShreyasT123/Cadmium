use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slotmap::SlotMap;

use crate::accessibility::{A11yAction, A11yNodeRecord, A11yRole, SemanticTree};
use crate::animation::Spring;
use crate::assets::{DecodedImage, cover_uv_rect, load_image_file, rasterize_svg};
use crate::core::{Color, CornerRadii, Rect};
use crate::debug::FrameProfiler;
use crate::events::{FocusManager, Key, Modifiers, WidgetId};
use crate::layout::{
    AlignItems, Edges, FlexDir, Inset, JustifyContent, LayoutEngine, LayoutNode, LayoutStyle,
    Length, PositionMode, snap_rect_to_pixels,
};
use crate::platform::Clipboard;
use crate::reactive::{EffectId, Signal, create_effect, dispose_effect};
use crate::renderer::{ATLAS_FORMAT, GlassQuad, ImageQuad, SdfQuad, TextureHandle};
use crate::style::{MacTheme, Material, Shadow, ThemeMode};
use crate::text::{FontWeight, TextBlock, TextEngine, TextStyle};
use crate::window::{CursorStyle, FrameContext, WindowCommand, WindowDelegate};

const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(530);
const TOOLTIP_HOVER_DELAY: Duration = Duration::from_millis(450);

#[derive(Debug, Clone, Copy, Default)]
pub struct DirtyFlags {
    pub state: bool,
    pub layout: bool,
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

pub struct MenuItem {
    pub label: Option<String>,
    pub on_select: Option<Box<dyn FnMut()>>,
}

impl MenuItem {
    pub fn new(label: impl Into<String>, on_select: impl FnMut() + 'static) -> Self {
        Self {
            label: Some(label.into()),
            on_select: Some(Box::new(on_select)),
        }
    }

    pub fn separator() -> Self {
        Self {
            label: None,
            on_select: None,
        }
    }
}

pub trait CustomGpuPass: 'static {
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

pub enum Element {
    Container(Container),
    Glass(GlassPanel),
    Text(Text),
    Image(Image),
    Icon(Icon),
    Button(Button),
    Switch(Switch),
    Slider(Slider),
    ProgressBar(ProgressBar),
    TextInput(TextInput),
    TextArea(TextArea),
    ScrollView(ScrollView),
    SplitPane(SplitPane),
    VirtualizedList(VirtualizedList),
    Switcher(Switcher),
    Show(Show),
    ForEach(ForEach),
    Tabs(Tabs),
    Dropdown(Dropdown),
    CustomGpuSurface(CustomGpuSurface),
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
    material_signal: Option<Signal<Material>>,
    radius: f32,
    radius_signal: Option<Signal<f32>>,
    is_drag_region: bool,
    tooltip: Option<String>,
    context_menu: Vec<MenuItem>,
    hover_fill: Option<Color>,
    on_click: Option<Box<dyn FnMut()>>,
    children: Vec<Element>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            layout: LayoutStyle::default(),
            material: Material::solid(Color::TRANSPARENT),
            material_signal: None,
            radius: 0.0,
            radius_signal: None,
            is_drag_region: false,
            tooltip: None,
            context_menu: Vec::new(),
            hover_fill: None,
            on_click: None,
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

    pub fn max_width(mut self, w: Length) -> Self {
        self.layout.max_width = w;
        self
    }

    pub fn max_height(mut self, h: Length) -> Self {
        self.layout.max_height = h;
        self
    }

    pub fn margin(mut self, m: Edges) -> Self {
        self.layout.margin = m;
        self
    }

    pub fn absolute(mut self) -> Self {
        self.layout.position = PositionMode::Absolute;
        self
    }

    pub fn inset(mut self, inset: Inset) -> Self {
        self.layout.inset = inset;
        self
    }

    pub fn hover_fill(mut self, color: Color) -> Self {
        self.hover_fill = Some(color);
        self
    }

    pub fn on_click(mut self, f: impl FnMut() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    pub fn bind_material(mut self, sig: Signal<Material>) -> Self {
        self.material = sig.try_get().unwrap_or(self.material);
        self.material_signal = Some(sig);
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

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn context_menu<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.context_menu.extend(items);
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

pub struct Stack;
impl Stack {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::new()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTone {
    Custom,
    Primary,
    Secondary,
    Tertiary,
    OnAccent,
}

impl TextTone {
    pub fn resolve(self, theme: &MacTheme, fallback: Color) -> Color {
        match self {
            Self::Custom => fallback,
            Self::Primary => theme.text_primary,
            Self::Secondary => theme.text_secondary,
            Self::Tertiary => theme.text_tertiary,
            Self::OnAccent => theme.text_on_accent,
        }
    }
}

enum TextSource {
    Static(String),
    Reactive(Box<dyn FnMut() -> String>),
}

pub struct Text {
    source: TextSource,
    style: TextStyle,
    tone: TextTone,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            source: TextSource::Static(text.into()),
            style: TextStyle::default(),
            tone: TextTone::Custom,
        }
    }

    pub fn reactive(f: impl FnMut() -> String + 'static) -> Self {
        Self {
            source: TextSource::Reactive(Box::new(f)),
            style: TextStyle::default(),
            tone: TextTone::Custom,
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

    pub fn tone(mut self, tone: TextTone) -> Self {
        self.tone = tone;
        self
    }
}

impl IntoElement for Text {
    fn into_element(self) -> Element {
        Element::Text(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFit {
    Cover,
    Contain,
    Stretch,
}

#[derive(Clone)]
pub enum ImageSource {
    File(PathBuf),
    Svg {
        data: String,
        width: u32,
        height: u32,
    },
    Decoded(Arc<DecodedImage>),
}

pub struct Image {
    source: ImageSource,
    layout: LayoutStyle,
    radius: f32,
    fit: ImageFit,
    tint: Color,
    shadow: Option<Shadow>,
    tooltip: Option<String>,
    context_menu: Vec<MenuItem>,
    on_click: Option<Box<dyn FnMut()>>,
}

impl Image {
    pub fn new(source: ImageSource) -> Self {
        Self {
            source,
            layout: LayoutStyle::default(),
            radius: 0.0,
            fit: ImageFit::Cover,
            tint: Color::from_rgba8(255, 255, 255, 255),
            shadow: None,
            tooltip: None,
            context_menu: Vec::new(),
            on_click: None,
        }
    }

    pub fn from_decoded(img: Arc<DecodedImage>) -> Self {
        Self::new(ImageSource::Decoded(img))
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.layout.flex_grow = g;
        self
    }
    pub fn from_file(path: impl Into<PathBuf>) -> Self {
        Self::new(ImageSource::File(path.into()))
    }

    pub fn svg(data: impl Into<String>, width: u32, height: u32) -> Self {
        Self::new(ImageSource::Svg {
            data: data.into(),
            width,
            height,
        })
    }

    pub fn width(mut self, w: Length) -> Self {
        self.layout.width = w;
        self
    }

    pub fn height(mut self, h: Length) -> Self {
        self.layout.height = h;
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = tint;
        self
    }

    pub fn shadow(mut self, color: Color, blur: f32, offset_y: f32) -> Self {
        self.shadow = Some(Shadow::new(color, blur, offset_y));
        self
    }

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn context_menu<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.context_menu.extend(items);
        self
    }

    pub fn on_click(mut self, f: impl FnMut() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl IntoElement for Image {
    fn into_element(self) -> Element {
        Element::Image(self)
    }
}

pub struct Icon {
    svg: String,
    size: f32,
    tone: TextTone,
    color: Color,
    tooltip: Option<String>,
}

impl Icon {
    pub fn svg(data: impl Into<String>, size: f32) -> Self {
        Self {
            svg: data.into(),
            size,
            color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            tone: TextTone::Primary,
            tooltip: None,
        }
    }
    pub fn color(mut self, color: Color) -> Self {
        self.tone = TextTone::Custom;
        self.color = color; // or `self.tint = color;` if the field is named `tint`
        self
    }
    pub fn tone(mut self, tone: TextTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for Icon {
    fn into_element(self) -> Element {
        Element::Icon(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Secondary,
    Primary,
    Ghost,
    Glass,
}

pub struct Button {
    label: String,
    accent: bool,
    variant: ButtonVariant,
    tooltip: Option<String>,
    on_click: Option<Box<dyn FnMut()>>,
    svg_icon: Option<String>,
    icon_size: f32,
    width: Length,
    height: Length,
    pill: bool,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            accent: false,
            variant: ButtonVariant::Secondary,
            tooltip: None,
            on_click: None,
            svg_icon: None,
            icon_size: 14.0,
            width: Length::Auto,
            height: Length::Px(30.0),
            pill: false,
        }
    }

    pub fn icon_only(svg: impl Into<String>, icon_size: f32) -> Self {
        let mut b = Self::new("");
        b.svg_icon = Some(svg.into());
        b.icon_size = icon_size;
        b.width = Length::Px(30.0);
        b.height = Length::Px(30.0);
        b
    }

    pub fn icon(mut self, svg: impl Into<String>, icon_size: f32) -> Self {
        self.svg_icon = Some(svg.into());
        self.icon_size = icon_size;
        self
    }

    pub fn primary(mut self) -> Self {
        self.accent = true;
        self.variant = ButtonVariant::Primary;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    pub fn glass(mut self) -> Self {
        self.variant = ButtonVariant::Glass;
        self
    }

    pub fn pill(mut self) -> Self {
        self.pill = true;
        self
    }

    pub fn size(mut self, w: Length, h: Length) -> Self {
        self.width = w;
        self.height = h;
        self
    }

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
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
    tooltip: Option<String>,
}

impl Switch {
    pub fn new(signal: Signal<bool>) -> Self {
        Self {
            signal,
            tooltip: None,
        }
    }

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
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
    tooltip: Option<String>,
}

impl Slider {
    pub fn new(signal: Signal<f32>, min: f32, max: f32) -> Self {
        Self {
            signal,
            min,
            max,
            width: 160.0,
            tooltip: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for Slider {
    fn into_element(self) -> Element {
        Element::Slider(self)
    }
}

pub struct ProgressBar {
    signal: Signal<f32>,
    width: Length,
    height: f32,
    color: Option<Color>,
}

impl ProgressBar {
    pub fn new(signal: Signal<f32>) -> Self {
        Self {
            signal,
            width: Length::Auto,
            height: 4.0,
            color: None,
        }
    }

    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = h.max(2.0);
        self
    }

    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}

impl IntoElement for ProgressBar {
    fn into_element(self) -> Element {
        Element::ProgressBar(self)
    }
}

pub struct TextInput {
    signal: Signal<String>,
    placeholder: String,
    material: Material,
    width: Length,
    tooltip: Option<String>,
}

impl TextInput {
    pub fn new(signal: Signal<String>) -> Self {
        Self {
            signal,
            placeholder: "Search or type...".into(),
            material: Material::solid(Color::from_rgba8(255, 255, 255, 240)),
            width: Length::Px(240.0),
            tooltip: None,
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

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for TextInput {
    fn into_element(self) -> Element {
        Element::TextInput(self)
    }
}

pub struct TextArea {
    signal: Signal<String>,
    placeholder: String,
    material: Material,
    width: Length,
    height: Length,
}

impl TextArea {
    pub fn new(signal: Signal<String>) -> Self {
        Self {
            signal,
            placeholder: "Enter multiline text or code...".into(),
            material: Material::Classic {
                fill: Color::from_rgba8(255, 255, 255, 220),
                fill_end: None,
                gradient_dir: [0.0, 1.0],
                border_color: Color::from_rgba8(0, 0, 0, 30),
                border_width: 1.0,
                shadow: None,
            },
            width: Length::Auto,
            height: Length::Px(160.0),
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

    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
}

impl IntoElement for TextArea {
    fn into_element(self) -> Element {
        Element::TextArea(self)
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
    tooltip: Option<String>,
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
            tooltip: None,
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

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
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
    tooltip: Option<String>,
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
            tooltip: None,
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

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for Dropdown {
    fn into_element(self) -> Element {
        Element::Dropdown(self)
    }
}

pub struct CustomGpuSurface {
    layout: LayoutStyle,
    radius: f32,
    radius_signal: Option<Signal<f32>>,
    animate_signal: Option<Signal<bool>>,
    tooltip: Option<String>,
    context_menu: Vec<MenuItem>,
    resolution_scale: f32,
    pass: Box<dyn CustomGpuPass>,
}

impl CustomGpuSurface {
    pub fn new(pass: impl CustomGpuPass) -> Self {
        Self {
            layout: LayoutStyle::default(),
            radius: 12.0,
            radius_signal: None,
            animate_signal: None,
            tooltip: None,
            context_menu: Vec::new(),
            resolution_scale: 1.0,
            pass: Box::new(pass),
        }
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

    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    pub fn bind_radius(mut self, sig: Signal<f32>) -> Self {
        self.radius = sig.try_get().unwrap_or(self.radius);
        self.radius_signal = Some(sig);
        self
    }

    pub fn animate_when(mut self, sig: Signal<bool>) -> Self {
        self.animate_signal = Some(sig);
        self
    }

    pub fn resolution_scale(mut self, s: f32) -> Self {
        self.resolution_scale = s.clamp(0.1, 1.0);
        self
    }

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn context_menu<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.context_menu.extend(items);
        self
    }
}

impl IntoElement for CustomGpuSurface {
    fn into_element(self) -> Element {
        Element::CustomGpuSurface(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

pub struct SplitPane {
    direction: SplitDirection,
    ratio: Signal<f32>,
    min_ratio: f32,
    max_ratio: f32,
    first: Option<Box<Element>>,
    second: Option<Box<Element>>,
    divider_color: Option<Color>,
}

impl SplitPane {
    pub fn horizontal(ratio: Signal<f32>) -> Self {
        Self::new(SplitDirection::Horizontal, ratio)
    }

    pub fn vertical(ratio: Signal<f32>) -> Self {
        Self::new(SplitDirection::Vertical, ratio)
    }

    pub fn new(direction: SplitDirection, ratio: Signal<f32>) -> Self {
        Self {
            direction,
            ratio,
            min_ratio: 0.15,
            max_ratio: 0.85,
            first: None,
            second: None,
            divider_color: None,
        }
    }

    pub fn bounds(mut self, min_ratio: f32, max_ratio: f32) -> Self {
        self.min_ratio = min_ratio.clamp(0.05, 0.90);
        self.max_ratio = max_ratio.clamp(self.min_ratio, 0.95);
        self
    }

    pub fn divider_color(mut self, color: Color) -> Self {
        self.divider_color = Some(color);
        self
    }

    pub fn first(mut self, elem: impl IntoElement) -> Self {
        self.first = Some(Box::new(elem.into_element()));
        self
    }

    pub fn second(mut self, elem: impl IntoElement) -> Self {
        self.second = Some(Box::new(elem.into_element()));
        self
    }
}

impl IntoElement for SplitPane {
    fn into_element(self) -> Element {
        Element::SplitPane(self)
    }
}

#[derive(Debug, Clone)]
pub struct VirtualRow {
    pub title: String,
    pub subtitle: String,
    pub badge: String,
}

pub struct VirtualizedList {
    item_count: usize,
    item_height: f32,
    gap: f32,
    selected: Option<Signal<usize>>,
    on_select: Option<Box<dyn FnMut(usize)>>,
    row_builder: Box<dyn FnMut(usize) -> VirtualRow>,
}

impl VirtualizedList {
    pub fn new(
        item_count: usize,
        item_height: f32,
        row_builder: impl FnMut(usize) -> VirtualRow + 'static,
    ) -> Self {
        Self {
            item_count,
            item_height: item_height.max(20.0),
            gap: 6.0,
            selected: None,
            on_select: None,
            row_builder: Box::new(row_builder),
        }
    }

    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g.max(0.0);
        self
    }

    pub fn selected(mut self, sig: Signal<usize>) -> Self {
        self.selected = Some(sig);
        self
    }

    pub fn on_select(mut self, f: impl FnMut(usize) + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }
}

impl IntoElement for VirtualizedList {
    fn into_element(self) -> Element {
        Element::VirtualizedList(self)
    }
}

pub struct Switcher {
    signal: Signal<usize>,
    children: Vec<Element>,
}

impl Switcher {
    pub fn new(signal: Signal<usize>, children: Vec<Element>) -> Self {
        Self { signal, children }
    }
}

impl IntoElement for Switcher {
    fn into_element(self) -> Element {
        Element::Switcher(self)
    }
}

pub struct Show {
    condition: Signal<bool>,
    when_true: Option<Box<dyn FnMut() -> Element>>,
    when_false: Option<Box<dyn FnMut() -> Element>>,
    layout: LayoutStyle,
}

impl Show {
    pub fn new(condition: Signal<bool>, when_true: impl FnMut() -> Element + 'static) -> Self {
        Self {
            condition,
            when_true: Some(Box::new(when_true)),
            when_false: None,
            layout: LayoutStyle::default(),
        }
    }

    pub fn fallback(mut self, when_false: impl FnMut() -> Element + 'static) -> Self {
        self.when_false = Some(Box::new(when_false));
        self
    }
}

impl IntoElement for Show {
    fn into_element(self) -> Element {
        Element::Show(self)
    }
}

type ForEachReconcile = Box<dyn FnMut(&mut WidgetArena, &mut TextEngine, WidgetId)>;

pub struct ForEach {
    layout: LayoutStyle,
    reconcile: ForEachReconcile,
    bind_signal: Box<dyn FnOnce(DirtyQueue, WidgetId) -> EffectId>,
}

impl ForEach {
    pub fn new<T: 'static>(
        items: Signal<Vec<T>>,
        mut key_fn: impl FnMut(&T) -> u64 + 'static,
        mut item_builder: impl FnMut(&T) -> Element + 'static,
    ) -> Self {
        let mut mounted_keys: HashMap<u64, WidgetId> = HashMap::new();

        let reconcile = Box::new(
            move |arena: &mut WidgetArena, text: &mut TextEngine, parent_id: WidgetId| {
                let mut next_children: Vec<WidgetId> = Vec::new();
                let mut next_map: HashMap<u64, WidgetId> = HashMap::new();

                let _ = items.try_with(|list| {
                    next_children.reserve(list.len());
                    next_map.reserve(list.len());
                    for item in list.iter() {
                        let k = key_fn(item);
                        let child_id = if let Some(existing) = mounted_keys.remove(&k) {
                            existing
                        } else {
                            let elem = item_builder(item);
                            let cid = arena.mount_node(elem, text);
                            if let Some(node) = arena.nodes.get_mut(cid) {
                                node.parent = Some(parent_id);
                            }
                            cid
                        };
                        next_children.push(child_id);
                        next_map.insert(k, child_id);
                    }
                });

                for (_, stale_id) in mounted_keys.drain() {
                    arena.unmount_subtree(stale_id);
                }
                mounted_keys = next_map;

                let child_lnodes: Vec<LayoutNode> = next_children
                    .iter()
                    .filter_map(|id| arena.nodes.get(*id).map(|n| n.layout_node))
                    .collect();
                if let Some(parent_lnode) = arena.nodes.get(parent_id).map(|n| n.layout_node) {
                    if let Some(node) = arena.nodes.get_mut(parent_id) {
                        node.children = next_children;
                    }
                    arena.layout.set_children(parent_lnode, &child_lnodes);
                    arena.layout.mark_dirty(parent_lnode);
                }
            },
        );

        let bind_signal = Box::new(move |q: DirtyQueue, id: WidgetId| {
            create_effect(move || {
                let _ = items.try_with(|_| ());
                push_dirty(
                    &q,
                    id,
                    DirtyFlags {
                        state: true,
                        layout: true,
                        paint: true,
                    },
                );
            })
        });

        Self {
            layout: LayoutStyle::default(),
            reconcile,
            bind_signal,
        }
    }

    pub fn direction(mut self, dir: FlexDir) -> Self {
        self.layout.direction = dir;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.layout.gap = gap;
        self
    }

    pub fn flex_grow(mut self, g: f32) -> Self {
        self.layout.flex_grow = g;
        self
    }
}

impl IntoElement for ForEach {
    fn into_element(self) -> Element {
        Element::ForEach(self)
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

struct CustomSurfaceState {
    pass: Box<dyn CustomGpuPass>,
    handle: Option<TextureHandle>,
    view: Option<wgpu::TextureView>,
    size: (u32, u32),
    rendered_once: bool,
}

struct ImageState {
    source_key: Option<usize>,
    image: Option<Arc<DecodedImage>>,
    handle: Option<TextureHandle>,
    width: u32,
    height: u32,
}

struct IconState {
    image: Option<DecodedImage>,
    handle: Option<TextureHandle>,
}

struct VirtualRowSlot {
    cached_index: Option<usize>,
    title: TextBlock,
    subtitle: TextBlock,
    badge: TextBlock,
    badge_width: f32,
}

struct VirtualListState {
    item_count: usize,
    item_height: f32,
    gap: f32,
    scroll_y: Spring,
    hovered_row: Option<usize>,
    selected: Option<Signal<usize>>,
    on_select: Option<Box<dyn FnMut(usize)>>,
    row_builder: Box<dyn FnMut(usize) -> VirtualRow>,
    pool: Vec<VirtualRowSlot>,
}

enum WidgetPayload {
    Container {
        material: Rc<Cell<Material>>,
        radius: Rc<Cell<f32>>,
        is_drag_region: bool,
        hover_fill: Option<Color>,
        on_click: Option<Box<dyn FnMut()>>,
    },
    Text {
        block: TextBlock,
        pending: Option<Rc<RefCell<String>>>,
        tone: TextTone,
    },
    Button {
        label: TextBlock,
        label_empty: bool,
        accent: bool,
        variant: ButtonVariant,
        icon_state: RefCell<IconState>,
        icon_size: f32,
        pill: bool,
        on_click: Option<Box<dyn FnMut()>>,
    },
    Image {
        state: RefCell<ImageState>,
        radius: f32,
        tint: Color,
        shadow: Option<Shadow>,
        fit: ImageFit,
        on_click: Option<Box<dyn FnMut()>>,
    },
    Icon {
        state: RefCell<IconState>,
        tone: TextTone,
        color: Color,
        size: f32,
    },
    Switch {
        signal: Signal<bool>,
    },
    Slider {
        signal: Signal<f32>,
        min: f32,
        max: f32,
    },
    ProgressBar {
        signal: Signal<f32>,
        color: Option<Color>,
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
    TextArea {
        signal: Signal<String>,
        material: Material,
        cursor: usize,
        selection: Option<(usize, usize)>,
        is_selecting: bool,
        text_block: TextBlock,
        placeholder_block: TextBlock,
        last_synced_text: String,
        scroll_y: f32,
    },
    ScrollView {
        scroll_y: Spring,
        content_height: f32,
        content_id: WidgetId,
    },
    SplitPane {
        direction: SplitDirection,
        ratio: Signal<f32>,
        min_ratio: f32,
        max_ratio: f32,
        first_id: WidgetId,
        divider_id: WidgetId,
        divider_color: Option<Color>,
    },
    VirtualizedList {
        state: RefCell<VirtualListState>,
    },
    Switcher {
        signal: Signal<usize>,
        children: Vec<WidgetId>,
    },
    Show {
        condition: Signal<bool>,
        active: bool,
        when_true: Option<Box<dyn FnMut() -> Element>>,
        when_false: Option<Box<dyn FnMut() -> Element>>,
    },
    ForEach {
        reconcile: ForEachReconcile,
    },
    Tabs {
        signal: Signal<usize>,
        items: Vec<String>,
        labels: Vec<TextBlock>,
        label_widths: Vec<f32>,
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
    CustomGpuSurface {
        radius: Rc<Cell<f32>>,
        animate_signal: Option<Signal<bool>>,
        resolution_scale: f32,
        state: RefCell<CustomSurfaceState>,
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
    tooltip: Option<String>,
    context_menu: Vec<MenuItem>,
    payload: WidgetPayload,
    parent: Option<WidgetId>,
    is_relayout_boundary: bool,
    dirty_layout_queued: bool,
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
    custom_surface_ids: Vec<WidgetId>,
    split_ids: Vec<WidgetId>,
    virtual_list_ids: Vec<WidgetId>,
    switcher_ids: Vec<WidgetId>,
    pending_texture_releases: Vec<TextureHandle>,
    dirty_layout_roots: Vec<WidgetId>,
    rect_buf: Vec<(LayoutNode, Rect)>,
    decoded_tex_cache: HashMap<usize, (TextureHandle, usize)>,
    svg_tex_cache: HashMap<(u64, u32, u32), (TextureHandle, usize)>,
    needs_full_layout: bool,
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
            custom_surface_ids: Vec::new(),
            split_ids: Vec::new(),
            virtual_list_ids: Vec::new(),
            switcher_ids: Vec::new(),
            pending_texture_releases: Vec::new(),
            dirty_layout_roots: Vec::new(),
            rect_buf: Vec::new(),
            decoded_tex_cache: HashMap::new(),
            svg_tex_cache: HashMap::new(),
            needs_full_layout: true,
        }
    }

    pub fn mount(&mut self, element: Element, text: &mut TextEngine) -> WidgetId {
        self.clear();
        let root_id = self.mount_node(element, text);
        self.root = Some(root_id);
        self.needs_full_layout = true;
        root_id
    }

    pub fn clear(&mut self) {
        let mut released: std::collections::HashSet<TextureHandle> =
            std::collections::HashSet::new();
        for (_, (handle, _)) in self.decoded_tex_cache.drain() {
            if released.insert(handle) {
                self.pending_texture_releases.push(handle);
            }
        }
        for (_, (handle, _)) in self.svg_tex_cache.drain() {
            if released.insert(handle) {
                self.pending_texture_releases.push(handle);
            }
        }

        for (_, node) in self.nodes.drain() {
            for eff in node.owned_effects {
                dispose_effect(eff);
            }
            match node.payload {
                WidgetPayload::CustomGpuSurface { state, .. } => {
                    if let Some(handle) = state.into_inner().handle
                        && released.insert(handle) {
                            self.pending_texture_releases.push(handle);
                        }
                }
                WidgetPayload::Image { state, .. } => {
                    let st = state.into_inner();
                    if let Some(handle) = st.handle {
                        let should_release = if let Some(key) = st.source_key {
                            if let Some((_, refs)) = self.decoded_tex_cache.get_mut(&key) {
                                *refs = refs.saturating_sub(1);
                                if *refs == 0 {
                                    self.decoded_tex_cache.remove(&key);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                true
                            }
                        } else {
                            true
                        };
                        if should_release && released.insert(handle) {
                            self.pending_texture_releases.push(handle);
                        }
                    }
                }
                WidgetPayload::Icon { state, .. } => {
                    if let Some(handle) = state.into_inner().handle
                        && released.insert(handle) {
                            self.pending_texture_releases.push(handle);
                        }
                }
                WidgetPayload::Button { icon_state, .. } => {
                    if let Some(handle) = icon_state.into_inner().handle
                        && released.insert(handle) {
                            self.pending_texture_releases.push(handle);
                        }
                }
                _ => {}
            }
            self.layout.remove(node.layout_node);
        }
        self.layout_to_widget.clear();
        self.dirty_queue.borrow_mut().clear();
        self.scroll_ids.clear();
        self.tab_ids.clear();
        self.traffic_ids.clear();
        self.custom_surface_ids.clear();
        self.split_ids.clear();
        self.virtual_list_ids.clear();
        self.switcher_ids.clear();
        for &id in &self.dirty_layout_roots {
            if let Some(n) = self.nodes.get_mut(id) {
                n.dirty_layout_queued = false;
            }
        }
        self.dirty_layout_roots.clear();
        self.focus = FocusManager::new();
        self.root = None;
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn unmount_subtree(&mut self, id: WidgetId) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        let children = node.children.clone();
        for child in children {
            self.unmount_subtree(child);
        }

        let Some(node) = self.nodes.remove(id) else {
            return;
        };

        for eff in node.owned_effects {
            dispose_effect(eff);
        }

        let mut released: std::collections::HashSet<TextureHandle> =
            std::collections::HashSet::new();
        match node.payload {
            WidgetPayload::CustomGpuSurface { state, .. } => {
                if let Some(handle) = state.into_inner().handle
                    && released.insert(handle) {
                        self.pending_texture_releases.push(handle);
                    }
            }
            WidgetPayload::Image { state, .. } => {
                let st = state.into_inner();
                if let Some(handle) = st.handle {
                    let should_release = if let Some(key) = st.source_key {
                        if let Some((_, refs)) = self.decoded_tex_cache.get_mut(&key) {
                            *refs = refs.saturating_sub(1);
                            if *refs == 0 {
                                self.decoded_tex_cache.remove(&key);
                                true
                            } else {
                                false
                            }
                        } else {
                            true
                        }
                    } else {
                        true
                    };
                    if should_release && released.insert(handle) {
                        self.pending_texture_releases.push(handle);
                    }
                }
            }
            WidgetPayload::Icon { state, .. } => {
                if let Some(handle) = state.into_inner().handle
                    && released.insert(handle) {
                        self.pending_texture_releases.push(handle);
                    }
            }
            WidgetPayload::Button { icon_state, .. } => {
                if let Some(handle) = icon_state.into_inner().handle
                    && released.insert(handle) {
                        self.pending_texture_releases.push(handle);
                    }
            }
            _ => {}
        }

        self.layout_to_widget.remove(&node.layout_node);
        self.layout.remove(node.layout_node);
        self.scroll_ids.retain(|&x| x != id);
        self.tab_ids.retain(|&x| x != id);
        self.traffic_ids.retain(|&x| x != id);
        self.custom_surface_ids.retain(|&x| x != id);
        self.split_ids.retain(|&x| x != id);
        self.virtual_list_ids.retain(|&x| x != id);
        self.switcher_ids.retain(|&x| x != id);
        self.dirty_layout_roots.retain(|&x| x != id);
    }

    pub fn bench_layout_pass(&mut self, width: f32, height: f32, text: &mut TextEngine) {
        let Some(root) = self.root else {
            return;
        };
        let root_lnode = self.nodes[root].layout_node;
        let mut rstyle = self.nodes[root].layout_style;
        rstyle.width = Length::Px(width);
        rstyle.height = Length::Px(height);
        self.layout.set_style(root_lnode, rstyle);

        let nodes = &mut self.nodes;
        let lmap = &self.layout_to_widget;
        let text_engine = text;
        self.layout
            .compute_with_measure(root_lnode, width, height, |lnode, _, _, avail_w, _| {
                let Some(&wid) = lmap.get(&lnode) else {
                    return (0.0, 0.0);
                };
                let Some(node) = nodes.get_mut(wid) else {
                    return (0.0, 0.0);
                };
                match &mut node.payload {
                    WidgetPayload::Text { block, .. } => block.measure(text_engine, avail_w),
                    WidgetPayload::TextArea { text_block, .. } => {
                        text_block.measure(text_engine, avail_w.map(|w| (w - 20.0).max(1.0)))
                    }
                    WidgetPayload::Button {
                        label,
                        label_empty,
                        icon_state,
                        icon_size,
                        ..
                    } => {
                        let (w, _) = label.measure(text_engine, None);
                        let has_icon = icon_state.borrow().handle.is_some()
                            || icon_state.borrow().image.is_some();
                        if has_icon && *label_empty {
                            (30.0, 30.0)
                        } else if has_icon {
                            (w + 28.0 + *icon_size, 30.0)
                        } else {
                            (w + 28.0, 30.0)
                        }
                    }
                    _ => (0.0, 0.0),
                }
            });

        let rects = self.layout.collect_logical_rects(root_lnode);
        for (lnode, rect) in rects {
            if let Some(&wid) = self.layout_to_widget.get(&lnode)
                && let Some(node) = self.nodes.get_mut(wid)
            {
                node.bounds = rect;
            }
        }
        self.needs_full_layout = false;
        for &id in &self.dirty_layout_roots {
            if let Some(n) = self.nodes.get_mut(id) {
                n.dirty_layout_queued = false;
            }
        }
        self.dirty_layout_roots.clear();
    }

    pub fn bench_sync_and_layout(&mut self, width: f32, height: f32, text: &mut TextEngine) {
        let dirty_batch: Vec<(WidgetId, DirtyFlags)> =
            std::mem::take(&mut *self.dirty_queue.borrow_mut());
        let mut foreach_dirty: Vec<WidgetId> = Vec::new();
        let mut show_dirty: Vec<WidgetId> = Vec::new();
        for (id, flags) in dirty_batch {
            let Some(node) = self.nodes.get(id) else { continue; };
            let layout_node = node.layout_node;
            let is_foreach = matches!(node.payload, WidgetPayload::ForEach { .. });
            let is_show = matches!(node.payload, WidgetPayload::Show { .. });

            if flags.layout {
                self.layout.mark_dirty(layout_node);
                let mut cur = id;
                while let Some(n) = self.nodes.get(cur) {
                    if n.is_relayout_boundary || n.parent.is_none() {
                        break;
                    }
                    if let Some(p) = n.parent {
                        cur = p;
                    } else {
                        break;
                    }
                }
                if Some(cur) == self.root {
                    self.needs_full_layout = true;
                } else if let Some(n) = self.nodes.get_mut(cur)
                    && !n.dirty_layout_queued
                {
                    n.dirty_layout_queued = true;
                    self.dirty_layout_roots.push(cur);
                }
            }

            if is_foreach && (flags.state || flags.layout) {
                foreach_dirty.push(id);
            }
            if is_show && (flags.state || flags.layout) {
                show_dirty.push(id);
            }
            if let Some(node) = self.nodes.get_mut(id) {
                    match &mut node.payload {
                        WidgetPayload::Text {
                            block,
                            pending: Some(cell),
                            ..
                        } => {
                            let text_val = cell.borrow().clone();
                            block.set_text(text, &text_val);
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
                            let text_val = signal.try_get().unwrap_or_default();
                            if *last_synced_text != text_val {
                                *last_synced_text = text_val.clone();
                                *cursor = snap_char_boundary(&text_val, *cursor);
                                *selection = normalize_selection(&text_val, *selection);
                                text_block.set_text(text, &text_val);
                                text_block.collect_char_offsets(char_offsets);
                            }
                        }
                        WidgetPayload::TextArea {
                            signal,
                            cursor,
                            selection,
                            text_block,
                            last_synced_text,
                            ..
                        } => {
                            let text_val = signal.try_get().unwrap_or_default();
                            if *last_synced_text != text_val {
                                *last_synced_text = text_val.clone();
                                *cursor = snap_char_boundary(&text_val, *cursor);
                                *selection = normalize_selection(&text_val, *selection);
                                text_block.set_text(text, &text_val);
                            }
                        }
                        _ => {}
                }
            }
        }

        for id in show_dirty {
            let (cond, active, mut when_true, mut when_false) =
                if let Some(node) = self.nodes.get_mut(id) {
                    if let WidgetPayload::Show {
                        condition,
                        active,
                        when_true,
                        when_false,
                    } = &mut node.payload
                    {
                        (*condition, *active, when_true.take(), when_false.take())
                    } else {
                        continue;
                    }
                } else {
                    continue;
                };
            let next = cond.try_get().unwrap_or(false);
            if next != active {
                let old_children: Vec<WidgetId> = self
                    .nodes
                    .get(id)
                    .map(|n| n.children.clone())
                    .unwrap_or_default();
                for child in old_children {
                    self.unmount_subtree(child);
                }
                let new_child = if next {
                    when_true.as_mut().map(|f| self.mount_node(f(), text))
                } else {
                    when_false.as_mut().map(|f| self.mount_node(f(), text))
                };
                let children: Vec<WidgetId> = new_child.into_iter().collect();
                for &child in &children {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }
                let child_lnodes: Vec<LayoutNode> = children
                    .iter()
                    .filter_map(|cid| self.nodes.get(*cid).map(|n| n.layout_node))
                    .collect();
                if let Some(node) = self.nodes.get_mut(id) {
                    node.children = children;
                    let lnode = node.layout_node;
                    if let WidgetPayload::Show {
                        active,
                        when_true: wt,
                        when_false: wf,
                        ..
                    } = &mut node.payload
                    {
                        *active = next;
                        *wt = when_true;
                        *wf = when_false;
                    }
                    self.layout.set_children(lnode, &child_lnodes);
                    self.layout.mark_dirty(lnode);
                }
                self.needs_full_layout = true;
            } else if let Some(node) = self.nodes.get_mut(id)
                && let WidgetPayload::Show {
                    when_true: wt,
                    when_false: wf,
                    ..
                } = &mut node.payload
            {
                *wt = when_true;
                *wf = when_false;
            }
        }

        for id in foreach_dirty {
            let reconcile_fn = if let Some(node) = self.nodes.get_mut(id) {
                if let WidgetPayload::ForEach { reconcile } = &mut node.payload {
                    Some(std::mem::replace(reconcile, Box::new(|_, _, _| {})))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(mut reconcile_fn) = reconcile_fn {
                reconcile_fn(self, text, id);
                if let Some(node) = self.nodes.get_mut(id)
                    && let WidgetPayload::ForEach { reconcile } = &mut node.payload
                {
                    *reconcile = reconcile_fn;
                }
                self.needs_full_layout = true;
            }
        }

        if self.needs_full_layout || self.dirty_layout_roots.len() > 32 {
            self.bench_layout_pass(width, height, text);
        } else if !self.dirty_layout_roots.is_empty() {
            let roots: Vec<WidgetId> = std::mem::take(&mut self.dirty_layout_roots);
            for boundary_id in roots {
                if let Some(n) = self.nodes.get_mut(boundary_id) {
                    n.dirty_layout_queued = false;
                }
                let Some(boundary_node) = self.nodes.get(boundary_id) else {
                    continue;
                };
                let boundary_lnode = boundary_node.layout_node;
                let bw = boundary_node.bounds.width.max(1.0);
                let bh = boundary_node.bounds.height.max(1.0);
                let origin_x = boundary_node.bounds.x;
                let origin_y = boundary_node.bounds.y;

                let nodes = &mut self.nodes;
                let lmap = &self.layout_to_widget;
                let text_engine: &mut TextEngine = &mut *text;
                self.layout.compute_subtree_with_measure(
                    boundary_lnode,
                    bw,
                    bh,
                    |lnode, _, _, avail_w, _| {
                        let Some(&wid) = lmap.get(&lnode) else {
                            return (0.0, 0.0);
                        };
                        let Some(node) = nodes.get_mut(wid) else {
                            return (0.0, 0.0);
                        };
                        match &mut node.payload {
                            WidgetPayload::Text { block, .. } => {
                                block.measure(text_engine, avail_w)
                            }
                            WidgetPayload::TextArea { text_block, .. } => text_block
                                .measure(text_engine, avail_w.map(|w| (w - 20.0).max(1.0))),
                            WidgetPayload::Button {
                                label,
                                label_empty,
                                icon_state,
                                icon_size,
                                ..
                            } => {
                                let (w, _) = label.measure(text_engine, None);
                                let has_icon = icon_state.borrow().handle.is_some()
                                    || icon_state.borrow().image.is_some();
                                if has_icon && *label_empty {
                                    (30.0, 30.0)
                                } else if has_icon {
                                    (w + 28.0 + *icon_size, 30.0)
                                } else {
                                    (w + 28.0, 30.0)
                                }
                            }
                            _ => (0.0, 0.0),
                        }
                    },
                );
                let slice =
                    self.layout
                        .collect_logical_rects_into(boundary_lnode, origin_x, origin_y);
                self.rect_buf.clear();
                self.rect_buf.extend_from_slice(slice);
                for i in 0..self.rect_buf.len() {
                    let (lnode, rect) = self.rect_buf[i];
                    if let Some(&wid) = self.layout_to_widget.get(&lnode)
                        && let Some(node) = self.nodes.get_mut(wid)
                    {
                        node.bounds = rect;
                    }
                }
            }
        }
    }

    pub fn bench_scroll_virtual_step(&mut self, delta_y: f32, text: &mut TextEngine) {
        for &vid in &self.virtual_list_ids {
            if let Some(node) = self.nodes.get(vid)
                && let WidgetPayload::VirtualizedList { state } = &node.payload
            {
                let mut st = state.borrow_mut();
                let stride = st.item_height + st.gap;
                let total_h = (st.item_count as f32 * stride - st.gap).max(0.0);
                let max_scroll = (total_h - node.bounds.height).max(0.0);
                let next = (st.scroll_y.value + delta_y) % max_scroll.max(1.0);
                st.scroll_y.snap_to(next);

                let first = (next / stride).floor().max(0.0) as usize;
                let visible_n = (node.bounds.height / stride).ceil() as usize + 2;
                let last = (first + visible_n).min(st.item_count);
                let pool_len = st.pool.len().max(1);
                for idx in first..last {
                    let slot_idx = idx % pool_len;
                    if st.pool[slot_idx].cached_index != Some(idx) {
                        let row_data = (st.row_builder)(idx);
                        let slot = &mut st.pool[slot_idx];
                        slot.cached_index = Some(idx);
                        slot.title.set_text(text, &row_data.title);
                        slot.subtitle.set_text(text, &row_data.subtitle);
                        slot.badge.set_text(text, &row_data.badge);
                        slot.badge_width = slot.badge.measure(text, None).0;
                    }
                }
            }
        }
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
                let is_boundary = matches!(c.layout.width, Length::Px(_))
                    && matches!(c.layout.height, Length::Px(_));
                let lnode = self.layout.new_container(c.layout, &child_layout_nodes);
                let radius_cell = Rc::new(Cell::new(c.radius));
                let material_cell = Rc::new(Cell::new(c.material));

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: is_boundary,
                    dirty_layout_queued: false,
                    children: child_ids.clone(),
                    layout_node: lnode,
                    layout_style: c.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: c.tooltip,
                    context_menu: c.context_menu,
                    payload: WidgetPayload::Container {
                        material: material_cell.clone(),
                        radius: radius_cell.clone(),
                        is_drag_region: c.is_drag_region,
                        hover_fill: c.hover_fill,
                        on_click: c.on_click,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                for &child in &child_ids {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }

                if let Some(sig) = c.radius_signal {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        if let Some(v) = sig.try_get() {
                            radius_cell.set(v);
                            push_dirty(
                                &q,
                                id,
                                DirtyFlags {
                                    state: false,
                                    layout: false,
                                    paint: true,
                                },
                            );
                        }
                    });
                    self.nodes[id].owned_effects.push(eff);
                }

                if let Some(sig) = c.material_signal {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        if let Some(m) = sig.try_get() {
                            material_cell.set(m);
                            push_dirty(
                                &q,
                                id,
                                DirtyFlags {
                                    state: false,
                                    layout: false,
                                    paint: true,
                                },
                            );
                        }
                    });
                    self.nodes[id].owned_effects.push(eff);
                }
                id
            }
            Element::Glass(g) => self.mount_node(Element::Container(g.inner), text),
            Element::Image(img) => {
                let (source_key, decoded) = match &img.source {
                    ImageSource::File(p) => (None, load_image_file(p).ok().map(Arc::new)),
                    ImageSource::Svg {
                        data,
                        width,
                        height,
                    } => (None, rasterize_svg(data, *width, *height).ok().map(Arc::new)),
                    ImageSource::Decoded(d) => (Some(Arc::as_ptr(d) as usize), Some(d.clone())),
                };
                let lnode = self.layout.new_leaf(img.layout);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: img.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: img.tooltip,
                    context_menu: img.context_menu,
                    payload: WidgetPayload::Image {
                        state: RefCell::new(ImageState {
                            source_key,
                            width: decoded.as_ref().map(|d| d.width).unwrap_or(1),
                            height: decoded.as_ref().map(|d| d.height).unwrap_or(1),
                            image: decoded,
                            handle: None,
                        }),
                        radius: img.radius,
                        tint: img.tint,
                        shadow: img.shadow,
                        fit: img.fit,
                        on_click: img.on_click,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                if matches!(
                    self.nodes[id].payload,
                    WidgetPayload::Image {
                        on_click: Some(_),
                        ..
                    }
                ) {
                    self.focus.focus_order.push(id);
                }
                id
            }
            Element::Icon(ic) => {
                let px = (ic.size * 2.0).round().max(1.0) as u32;
                let decoded = rasterize_svg(&ic.svg, px, px).ok();
                let lstyle = LayoutStyle {
                    width: Length::Px(ic.size),
                    height: Length::Px(ic.size),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: ic.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Icon {
                        state: RefCell::new(IconState {
                            image: decoded,
                            handle: None,
                        }),
                        tone: ic.tone,
                        color: ic.color,
                        size: ic.size,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                id
            }
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
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Text {
                        block,
                        pending: if is_reactive {
                            Some(shared.clone())
                        } else {
                            None
                        },
                        tone: t.tone,
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
                let icon_image = b.svg_icon.as_ref().and_then(|svg| {
                    let px = (b.icon_size * 2.0).round().max(1.0) as u32;
                    rasterize_svg(svg, px, px).ok()
                });
                let label_block = TextBlock::new(
                    text,
                    &b.label,
                    TextStyle {
                        font_size: 13.0,
                        line_height: 18.0,
                        color: Color::from_rgba8(29, 29, 31, 255),
                        weight: FontWeight::Medium,
                    },
                );
                let lstyle = LayoutStyle {
                    width: b.width,
                    height: b.height,
                    padding: if b.svg_icon.is_some() && b.label.is_empty() {
                        Edges::all(0.0)
                    } else {
                        Edges::xy(14.0, 6.0)
                    },
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: b.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Button {
                        label: label_block,
                        label_empty: b.label.is_empty(),
                        accent: b.accent,
                        variant: b.variant,
                        icon_state: RefCell::new(IconState {
                            image: icon_image,
                            handle: None,
                        }),
                        icon_size: b.icon_size,
                        pill: b.pill,
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
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: sw.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Switch { signal: sw.signal },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = sw.signal.try_get();
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
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: sl.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Slider {
                        signal: sl.signal,
                        min: sl.min,
                        max: sl.max,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = sl.signal.try_get();
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
            Element::ProgressBar(pb) => {
                let lstyle = LayoutStyle {
                    width: pb.width,
                    height: Length::Px(pb.height),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::ProgressBar {
                        signal: pb.signal,
                        color: pb.color,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = pb.signal.try_get();
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
                let current_val = ti.signal.try_get().unwrap_or_default();
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
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: ti.tooltip,
                    context_menu: Vec::new(),
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
                    let _ = ti.signal.try_get();
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
            Element::TextArea(ta) => {
                let lstyle = LayoutStyle {
                    width: ta.width,
                    height: ta.height,
                    padding: Edges::all(10.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let current_val = ta.signal.try_get().unwrap_or_default();
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
                    &ta.placeholder,
                    TextStyle {
                        font_size: 13.5,
                        line_height: 18.0,
                        color: Color::from_rgba8(142, 142, 147, 255),
                        weight: FontWeight::Regular,
                    },
                );
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::TextArea {
                        signal: ta.signal,
                        material: ta.material,
                        cursor: current_val.len(),
                        selection: None,
                        is_selecting: false,
                        text_block,
                        placeholder_block,
                        last_synced_text: current_val,
                        scroll_y: 0.0,
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = ta.signal.try_get();
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
            Element::Switcher(sw) => {
                let child_ids: Vec<WidgetId> = sw
                    .children
                    .into_iter()
                    .map(|ch| self.mount_node(ch, text))
                    .collect();
                let child_layout_nodes: Vec<LayoutNode> = child_ids
                    .iter()
                    .map(|id| self.nodes[*id].layout_node)
                    .collect();
                let lstyle = LayoutStyle {
                    flex_grow: 1.0,
                    ..Default::default()
                };
                let lnode = self.layout.new_container(lstyle, &child_layout_nodes);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: child_ids.clone(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Switcher {
                        signal: sw.signal,
                        children: child_ids.clone(),
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                for &child in &child_ids {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }

                let active = sw.signal.try_get().unwrap_or(0);
                for (i, &cid) in child_ids.iter().enumerate() {
                    if let Some(node) = self.nodes.get_mut(cid) {
                        node.layout_style.hidden = i != active;
                        let style = node.layout_style;
                        let ln = node.layout_node;
                        self.layout.set_style(ln, style);
                    }
                }

                let q = self.dirty_queue.clone();
                let children_for_eff = child_ids.clone();
                let eff = create_effect(move || {
                    let active = sw.signal.try_get().unwrap_or(0);
                    // We'll mark layout dirty; actual hidden toggling happens in sync_switchers.
                    let _ = active;
                    let _ = &children_for_eff;
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: true,
                            layout: true,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                self.switcher_ids.push(id);
                id
            }
            Element::Show(mut sh) => {
                let condition = sh.condition;
                let initial = condition.try_get().unwrap_or(false);
                let mut active_child = None;
                let child_id = if initial {
                    sh.when_true.as_mut().map(|f| {
                        let cid = self.mount_node(f(), text);
                        active_child = Some(cid);
                        cid
                    })
                } else {
                    sh.when_false.as_mut().map(|f| {
                        let cid = self.mount_node(f(), text);
                        active_child = Some(cid);
                        cid
                    })
                };
                let children: Vec<WidgetId> = child_id.into_iter().collect();
                let child_lnodes: Vec<LayoutNode> = children
                    .iter()
                    .filter_map(|cid| self.nodes.get(*cid).map(|n| n.layout_node))
                    .collect();
                let lnode = self.layout.new_container(sh.layout, &child_lnodes);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: children.clone(),
                    layout_node: lnode,
                    layout_style: sh.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Show {
                        condition,
                        active: initial,
                        when_true: sh.when_true,
                        when_false: sh.when_false,
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                for &child in &children {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = condition.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: true,
                            layout: true,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                id
            }
            Element::ForEach(mut fe) => {
                let lnode = self.layout.new_container(fe.layout, &[]);
                let id = self.nodes.insert(WidgetNode {
                    children: Vec::new(),
                    parent: None,
                    is_relayout_boundary: matches!(
                        (fe.layout.width, fe.layout.height),
                        (Length::Px(_), Length::Px(_))
                    ),
                    dirty_layout_queued: false,
                    layout_node: lnode,
                    layout_style: fe.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::ForEach {
                        reconcile: Box::new(|_, _, _| {}),
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                let eff = (fe.bind_signal)(self.dirty_queue.clone(), id);
                self.nodes[id].owned_effects.push(eff);
                // Populate initial children immediately on mount!
                (fe.reconcile)(self, text, id);
                if let Some(node) = self.nodes.get_mut(id)
                    && let WidgetPayload::ForEach { reconcile } = &mut node.payload
                {
                    *reconcile = fe.reconcile;
                }
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

                let mut labels = Vec::with_capacity(tb.items.len());
                let mut label_widths = Vec::with_capacity(tb.items.len());
                for it in &tb.items {
                    let mut block = TextBlock::new(
                        text,
                        it,
                        TextStyle {
                            font_size: 12.5,
                            line_height: 16.0,
                            color: Color::from_rgba8(28, 28, 32, 255),
                            weight: FontWeight::Medium,
                        },
                    );
                    let (w, _) = block.measure(text, None);
                    labels.push(block);
                    label_widths.push(w);
                }

                let initial_tab = tb.signal.try_get().unwrap_or(0);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: tb.tooltip,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Tabs {
                        signal: tb.signal,
                        items: tb.items,
                        labels,
                        label_widths,
                        material: tb.material,
                        pill_spring: Spring::new(initial_tab as f32, 320.0, 26.0),
                    },
                });
                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = tb.signal.try_get();
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
                    "v",
                    TextStyle {
                        font_size: 11.0,
                        line_height: 14.0,
                        color: Color::from_rgba8(120, 120, 128, 255),
                        weight: FontWeight::Bold,
                    },
                );

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: dd.tooltip,
                    context_menu: Vec::new(),
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
                    let _ = dd.signal.try_get();
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
            Element::CustomGpuSurface(cs) => {
                let lnode = self.layout.new_leaf(cs.layout);
                let radius_cell = Rc::new(Cell::new(cs.radius));
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: cs.layout,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: cs.tooltip,
                    context_menu: cs.context_menu,
                    payload: WidgetPayload::CustomGpuSurface {
                        radius: radius_cell.clone(),
                        animate_signal: cs.animate_signal,
                        resolution_scale: cs.resolution_scale,
                        state: RefCell::new(CustomSurfaceState {
                            pass: cs.pass,
                            handle: None,
                            view: None,
                            size: (0, 0),
                            rendered_once: false,
                        }),
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                self.custom_surface_ids.push(id);

                if let Some(sig) = cs.radius_signal {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        if let Some(v) = sig.try_get() {
                            radius_cell.set(v);
                            push_dirty(
                                &q,
                                id,
                                DirtyFlags {
                                    state: false,
                                    layout: false,
                                    paint: true,
                                },
                            );
                        }
                    });
                    self.nodes[id].owned_effects.push(eff);
                }
                if let Some(anim_sig) = cs.animate_signal {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        let _ = anim_sig.try_get();
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
            Element::TrafficLights(_) => {
                let lstyle = LayoutStyle {
                    width: Length::Px(56.0),
                    height: Length::Px(16.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
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
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: child_ids.clone(),
                    layout_node: content_lnode,
                    layout_style: content_lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Container {
                        material: Rc::new(Cell::new(Material::solid(Color::TRANSPARENT))),
                        radius: Rc::new(Cell::new(0.0)),
                        is_drag_region: false,
                        hover_fill: None,
                        on_click: None,
                    },
                });
                self.layout_to_widget.insert(content_lnode, content_id);
                for &child in &child_ids {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(content_id);
                    }
                }

                let viewport_lstyle = LayoutStyle {
                    direction: FlexDir::Column,
                    flex_grow: 1.0,
                    ..Default::default()
                };
                let viewport_lnode = self.layout.new_container(viewport_lstyle, &[content_lnode]);

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: vec![content_id],
                    layout_node: viewport_lnode,
                    layout_style: viewport_lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::ScrollView {
                        scroll_y: Spring::new(0.0, 260.0, 24.0),
                        content_height: 0.0,
                        content_id,
                    },
                });
                self.layout_to_widget.insert(viewport_lnode, id);
                if let Some(node) = self.nodes.get_mut(content_id) {
                    node.parent = Some(id);
                }
                self.scroll_ids.push(id);
                id
            }
            Element::SplitPane(sp) => {
                let first_elem = sp
                    .first
                    .map(|b| *b)
                    .unwrap_or_else(|| Spacer::flex().into_element());
                let second_elem = sp
                    .second
                    .map(|b| *b)
                    .unwrap_or_else(|| Spacer::flex().into_element());

                let first_id = self.mount_node(first_elem, text);
                let second_id = self.mount_node(second_elem, text);

                let init_ratio = sp
                    .ratio
                    .try_get()
                    .unwrap_or(0.3)
                    .clamp(sp.min_ratio, sp.max_ratio);

                let is_h = sp.direction == SplitDirection::Horizontal;

                let mut first_style = self.nodes[first_id].layout_style;
                if is_h {
                    first_style.width = Length::Percent(init_ratio);
                    first_style.height = Length::Auto;
                } else {
                    first_style.height = Length::Percent(init_ratio);
                    first_style.width = Length::Auto;
                }
                first_style.flex_grow = 0.0;
                first_style.flex_shrink = 0.0;
                self.nodes[first_id].layout_style = first_style;
                self.layout
                    .set_style(self.nodes[first_id].layout_node, first_style);

                let div_style = LayoutStyle {
                    width: if is_h { Length::Px(1.0) } else { Length::Auto },
                    height: if is_h { Length::Auto } else { Length::Px(1.0) },
                    flex_shrink: 0.0,
                    ..Default::default()
                };
                let div_lnode = self.layout.new_leaf(div_style);
                let divider_id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: div_lnode,
                    layout_style: div_style,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::Spacer,
                });
                self.layout_to_widget.insert(div_lnode, divider_id);

                let mut second_style = self.nodes[second_id].layout_style;
                second_style.flex_grow = 1.0;
                second_style.flex_shrink = 1.0;
                self.nodes[second_id].layout_style = second_style;
                self.layout
                    .set_style(self.nodes[second_id].layout_node, second_style);

                let parent_style = LayoutStyle {
                    direction: if is_h { FlexDir::Row } else { FlexDir::Column },
                    flex_grow: 1.0,
                    ..Default::default()
                };
                let child_lnodes = [
                    self.nodes[first_id].layout_node,
                    div_lnode,
                    self.nodes[second_id].layout_node,
                ];
                let parent_lnode = self.layout.new_container(parent_style, &child_lnodes);

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: vec![first_id, divider_id, second_id],
                    layout_node: parent_lnode,
                    layout_style: parent_style,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::SplitPane {
                        direction: sp.direction,
                        ratio: sp.ratio,
                        min_ratio: sp.min_ratio,
                        max_ratio: sp.max_ratio,
                        first_id,
                        divider_id,
                        divider_color: sp.divider_color,
                    },
                });
                self.layout_to_widget.insert(parent_lnode, id);
                for &child in &[first_id, divider_id, second_id] {
                    if let Some(node) = self.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }
                self.split_ids.push(id);

                let q = self.dirty_queue.clone();
                let eff = create_effect(move || {
                    let _ = sp.ratio.try_get();
                    push_dirty(
                        &q,
                        id,
                        DirtyFlags {
                            state: true,
                            layout: true,
                            paint: true,
                        },
                    );
                });
                self.nodes[id].owned_effects.push(eff);
                id
            }
            Element::VirtualizedList(vl) => {
                let lstyle = LayoutStyle {
                    flex_grow: 1.0,
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);

                let pool = (0..40)
                    .map(|_| VirtualRowSlot {
                        cached_index: None,
                        title: TextBlock::new(
                            text,
                            "",
                            TextStyle {
                                font_size: 13.0,
                                line_height: 18.0,
                                color: Color::from_rgba8(29, 29, 31, 255),
                                weight: FontWeight::Medium,
                            },
                        ),
                        subtitle: TextBlock::new(
                            text,
                            "",
                            TextStyle {
                                font_size: 11.5,
                                line_height: 15.0,
                                color: Color::from_rgba8(110, 110, 115, 255),
                                weight: FontWeight::Regular,
                            },
                        ),
                        badge: TextBlock::new(
                            text,
                            "",
                            TextStyle {
                                font_size: 12.0,
                                line_height: 16.0,
                                color: Color::from_rgba8(110, 110, 115, 255),
                                weight: FontWeight::Medium,
                            },
                        ),
                        badge_width: 0.0,
                    })
                    .collect();

                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: true,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
                    payload: WidgetPayload::VirtualizedList {
                        state: RefCell::new(VirtualListState {
                            item_count: vl.item_count,
                            item_height: vl.item_height,
                            gap: vl.gap,
                            scroll_y: Spring::new(0.0, 260.0, 24.0),
                            hovered_row: None,
                            selected: vl.selected,
                            on_select: vl.on_select,
                            row_builder: vl.row_builder,
                            pool,
                        }),
                    },
                });
                self.layout_to_widget.insert(lnode, id);
                self.virtual_list_ids.push(id);

                if let Some(sel) = vl.selected {
                    let q = self.dirty_queue.clone();
                    let eff = create_effect(move || {
                        let _ = sel.try_get();
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
            Element::Divider(d) => {
                let lstyle = LayoutStyle {
                    height: Length::Px(1.0),
                    ..Default::default()
                };
                let lnode = self.layout.new_leaf(lstyle);
                let id = self.nodes.insert(WidgetNode {
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
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
                    parent: None,
                    is_relayout_boundary: false,
                    dirty_layout_queued: false,
                    children: Vec::new(),
                    layout_node: lnode,
                    layout_style: lstyle,
                    bounds: Rect::default(),
                    owned_effects: Vec::new(),
                    tooltip: None,
                    context_menu: Vec::new(),
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

struct ContextMenuState {
    owner: WidgetId,
    anchor: (f32, f32),
    menu_rect: Rect,
    hovered_idx: Option<usize>,
    labels: Vec<Option<(TextBlock, TextBlock)>>,
}

struct PaintCtx<'a, 'b> {
    ctx: &'a mut FrameContext<'b>,
    theme: &'a MacTheme,
    scale: f32,
    time_secs: f32,
    caret_visible: bool,
    hovered: Option<WidgetId>,
    focused: Option<WidgetId>,
    overlays: &'a mut Vec<OverlayRenderRecord>,
    a11y_records: Option<&'a mut Vec<A11yNodeRecord>>,
    selection_scratch: &'a mut Vec<(f32, f32, f32)>,
    decoded_tex_cache: &'a mut HashMap<usize, (TextureHandle, usize)>,
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
    hovered_dropdown_row: Option<(WidgetId, usize)>,
    context_menu: Option<ContextMenuState>,
    tooltip_id: Option<WidgetId>,
    tooltip_anchor: Rect,
    tooltip_show_at: Option<Instant>,
    tooltip_visible: bool,
    tooltip_cached_id: Option<WidgetId>,
    tooltip_block: Option<TextBlock>,
    caret_visible: bool,
    next_blink_at: Option<Instant>,
    start_instant: Instant,
    show_fps: bool,
    fps_block: Option<TextBlock>,
    was_animating: bool,
    path_draw_fn: Option<PathDrawFn>,
    last_fps_update: Instant,
    last_cursor: (f32, f32),
    a11y_active: bool,
    cached_semantic_tree: Option<SemanticTree>,
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
            hovered_dropdown_row: None,
            context_menu: None,
            tooltip_id: None,
            tooltip_anchor: Rect::default(),
            tooltip_show_at: None,
            tooltip_visible: false,
            tooltip_cached_id: None,
            tooltip_block: None,
            caret_visible: true,
            next_blink_at: None,
            start_instant: Instant::now(),
            show_fps: false,
            fps_block: None,
            was_animating: false,
            path_draw_fn: None,
            last_fps_update: Instant::now(),
            last_cursor: (0.0, 0.0),
            a11y_active: false,
            cached_semantic_tree: None,
        }
    }

    pub fn with_theme(mut self, theme_signal: Signal<ThemeMode>) -> Self {
        self.theme_signal = Some(theme_signal);
        self
    }

    pub fn show_fps(mut self, show: bool) -> Self {
        self.show_fps = show;
        self
    }

    pub fn draw_paths(mut self, f: impl FnMut(&mut FrameContext<'_>) + 'static) -> Self {
        self.path_draw_fn = Some(Box::new(f));
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

    fn clear_tooltip(&mut self) -> bool {
        let was_visible = self.tooltip_visible;
        self.tooltip_id = None;
        self.tooltip_show_at = None;
        self.tooltip_visible = false;
        was_visible
    }

    fn reset_caret_blink(&mut self) {
        let is_text_focused = self
            .arena
            .focus
            .focused
            .and_then(|id| self.arena.nodes.get(id))
            .is_some_and(|n| {
                matches!(
                    n.payload,
                    WidgetPayload::TextInput { .. } | WidgetPayload::TextArea { .. }
                )
            });

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
        self.hovered_dropdown_row = None;
        self.context_menu = None;
        self.tooltip_id = None;
        self.tooltip_show_at = None;
        self.tooltip_visible = false;
        self.tooltip_cached_id = None;
        self.tooltip_block = None;
        self.caret_visible = false;
        self.next_blink_at = None;
        self.cached_semantic_tree = None;

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
                pill_spring.set_target(signal.try_get().unwrap_or(0) as f32);
                animating |= pill_spring.step(dt);
            }
        }
        for &cid in &self.arena.custom_surface_ids {
            if let Some(node) = self.arena.nodes.get(cid)
                && let WidgetPayload::CustomGpuSurface {
                    animate_signal: Some(sig),
                    ..
                } = &node.payload
                && sig.try_get().unwrap_or(false)
            {
                animating = true;
            }
        }
        for &vid in &self.arena.virtual_list_ids {
            if let Some(node) = self.arena.nodes.get(vid)
                && let WidgetPayload::VirtualizedList { state } = &node.payload
            {
                animating |= state.borrow_mut().scroll_y.step(dt);
            }
        }
        for &sid in &self.arena.switcher_ids.clone() {
            let (active, children) = if let Some(node) = self.arena.nodes.get(sid)
                && let WidgetPayload::Switcher { signal, children } = &node.payload
            {
                (signal.try_get().unwrap_or(0), children.clone())
            } else {
                continue;
            };
            for (i, &cid) in children.iter().enumerate() {
                let hidden = i != active;
                if let Some(node) = self.arena.nodes.get_mut(cid)
                    && node.layout_style.hidden != hidden
                {
                    node.layout_style.hidden = hidden;
                    let style = node.layout_style;
                    let ln = node.layout_node;
                    self.arena.layout.set_style(ln, style);
                    self.arena.needs_full_layout = true;
                }
            }
        }

        let now = Instant::now();
        if let Some(blink_at) = self.next_blink_at
            && now >= blink_at
        {
            self.caret_visible = !self.caret_visible;
            self.next_blink_at = Some(now + CARET_BLINK_INTERVAL);
        }

        if let Some(show_at) = self.tooltip_show_at
            && now >= show_at
        {
            self.tooltip_visible = true;
            self.tooltip_show_at = None;
        }

        self.was_animating = animating;
        animating
    }

    fn next_redraw_at(&self) -> Option<Instant> {
        match (self.next_blink_at, self.tooltip_show_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        if let Some(target) = self.captured.or(self.hovered)
            && let Some(node) = self.arena.nodes.get(target)
        {
            match node.payload {
                WidgetPayload::SplitPane {
                    direction: SplitDirection::Horizontal,
                    ..
                } => return CursorStyle::ColResize,
                WidgetPayload::SplitPane {
                    direction: SplitDirection::Vertical,
                    ..
                } => return CursorStyle::RowResize,
                WidgetPayload::TextInput { .. } | WidgetPayload::TextArea { .. } => {
                    return CursorStyle::Default;
                }
                _ => return CursorStyle::Pointer,
            }
        }
        CursorStyle::Default
    }

    fn is_drag_region(&self, x: f32, y: f32) -> bool {
        if self.context_menu.is_some() || self.hit_test(x, y).is_some() {
            return false;
        }
        self.arena
            .root
            .map(|root| is_drag_node(root, &self.arena.nodes, x, y, (0.0, 0.0)))
            .unwrap_or(false)
    }

    fn on_scroll(&mut self, _delta_x: f32, delta_y: f32) -> bool {
        let mut handled = self.clear_tooltip();
        let (cx, cy) = self.last_cursor;
        if let Some(root) = self.arena.root {
            handled |=
                dispatch_scroll_node(root, &mut self.arena.nodes, cx, cy, delta_y, (0.0, 0.0));
        }
        handled
    }

    fn set_a11y_active(&mut self, active: bool) {
        self.a11y_active = active;
        if !active {
            self.cached_semantic_tree = None;
        }
    }

    fn semantic_tree(&self) -> Option<SemanticTree> {
        self.cached_semantic_tree.clone()
    }

    fn on_semantic_action(&mut self, action: A11yAction) -> bool {
        match action {
            A11yAction::Focus(wid) => {
                if self.arena.nodes.contains_key(wid) {
                    let changed = self.arena.focus.set_focus(Some(wid));
                    self.reset_caret_blink();
                    return changed;
                }
                false
            }
            A11yAction::Click(wid) => {
                let Some(node) = self.arena.nodes.get_mut(wid) else {
                    return false;
                };
                match &mut node.payload {
                    WidgetPayload::Button { on_click, .. } => {
                        if let Some(cb) = on_click {
                            cb();
                        }
                        true
                    }
                    WidgetPayload::Switch { signal } => {
                        signal.update(|v| *v = !*v);
                        true
                    }
                    WidgetPayload::Dropdown { is_open, .. } => {
                        *is_open = !*is_open;
                        true
                    }
                    WidgetPayload::TextInput { .. } => {
                        self.arena.focus.set_focus(Some(wid));
                        self.reset_caret_blink();
                        true
                    }
                    _ => false,
                }
            }
        }
    }

    fn on_key(&mut self, key: &Key, text: Option<&str>, modifiers: &Modifiers) -> bool {
        if *key == Key::Escape && self.context_menu.take().is_some() {
            return true;
        }
        if *key == Key::Tab {
            if modifiers.shift {
                self.arena.focus.cycle_prev();
            } else {
                self.arena.focus.cycle_next();
            }
            self.reset_caret_blink();
            return true;
        }

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
            let Some(mut val) = signal.try_get() else {
                return false;
            };
            *cursor = snap_char_boundary(&val, *cursor);
            *selection = normalize_selection(&val, *selection);

            let is_ctrl = modifiers.ctrl || modifiers.super_key;
            let is_shortcut_char = |ch: &str| -> bool {
                matches!(key, Key::Character(c) if c.eq_ignore_ascii_case(ch))
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
                    Key::Backspace => {
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
                    Key::Delete => {
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
                    Key::ArrowLeft => {
                        *selection = None;
                        let next = if is_ctrl {
                            word_prev(&val, *cursor)
                        } else {
                            val[..*cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0)
                        };
                        if next != *cursor {
                            *cursor = next;
                            handled = true;
                        }
                    }
                    Key::ArrowRight => {
                        *selection = None;
                        let next = if is_ctrl {
                            word_next(&val, *cursor)
                        } else {
                            val[*cursor..]
                                .char_indices()
                                .nth(1)
                                .map(|(i, _)| *cursor + i)
                                .unwrap_or(val.len())
                        };
                        if next != *cursor {
                            *cursor = next;
                            handled = true;
                        }
                    }
                    Key::Home => {
                        *selection = None;
                        *cursor = 0;
                        handled = true;
                    }
                    Key::End => {
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

        if !handled
            && let WidgetPayload::TextArea {
                signal,
                cursor,
                selection,
                text_block,
                ..
            } = &mut node.payload
        {
            let Some(mut val) = signal.try_get() else {
                return false;
            };
            *cursor = snap_char_boundary(&val, *cursor);
            *selection = normalize_selection(&val, *selection);

            let is_ctrl = modifiers.ctrl || modifiers.super_key;
            let is_shortcut_char = |ch: &str| -> bool {
                matches!(key, Key::Character(c) if c.eq_ignore_ascii_case(ch))
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
                    delete_sel(&mut val, cursor, selection);
                    val.insert_str(*cursor, &pasted);
                    *cursor += pasted.len();
                    signal.set(val);
                    handled = true;
                }
            } else {
                match key {
                    Key::Enter => {
                        delete_sel(&mut val, cursor, selection);
                        val.insert(*cursor, '\n');
                        *cursor += 1;
                        signal.set(val);
                        handled = true;
                    }
                    Key::Backspace => {
                        if delete_sel(&mut val, cursor, selection) {
                            signal.set(val);
                            handled = true;
                        } else if *cursor > 0 {
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
                    Key::Delete => {
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
                    Key::ArrowLeft => {
                        *selection = None;
                        let next = if is_ctrl {
                            word_prev(&val, *cursor)
                        } else {
                            val[..*cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0)
                        };
                        if next != *cursor {
                            *cursor = next;
                            handled = true;
                        }
                    }
                    Key::ArrowRight => {
                        *selection = None;
                        let next = if is_ctrl {
                            word_next(&val, *cursor)
                        } else {
                            val[*cursor..]
                                .char_indices()
                                .nth(1)
                                .map(|(i, _)| *cursor + i)
                                .unwrap_or(val.len())
                        };
                        if next != *cursor {
                            *cursor = next;
                            handled = true;
                        }
                    }
                    Key::ArrowUp | Key::ArrowDown => {
                        let delta = if *key == Key::ArrowUp { -1 } else { 1 };
                        *selection = None;
                        *cursor =
                            snap_char_boundary(&val, text_block.move_vertical_byte(*cursor, delta));
                        handled = true;
                    }
                    Key::Home => {
                        *selection = None;
                        *cursor = line_range(&val, line_index_at(&val, *cursor)).0;
                        handled = true;
                    }
                    Key::End => {
                        *selection = None;
                        *cursor = line_range(&val, line_index_at(&val, *cursor)).1;
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
            if matches!(node.payload, WidgetPayload::TextArea { .. }) {
                ensure_textarea_caret_visible(node);
            }
            self.reset_caret_blink();
        }
        handled
    }

    fn on_pointer_move(&mut self, x: f32, y: f32) -> bool {
        self.last_cursor = (x, y);
        if let Some(cap) = self.captured
            && let Some(node) = self.arena.nodes.get(cap)
            && let WidgetPayload::SplitPane {
                direction,
                ratio,
                min_ratio,
                max_ratio,
                ..
            } = &node.payload
        {
            let next_r = match direction {
                SplitDirection::Horizontal => (x - node.bounds.x) / node.bounds.width.max(1.0),
                SplitDirection::Vertical => (y - node.bounds.y) / node.bounds.height.max(1.0),
            }
            .clamp(*min_ratio, *max_ratio);
            ratio.set(next_r);
            return true;
        }
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
            let val = signal.try_get().unwrap_or_default();
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
        if let Some(cap) = self.captured
            && let Some(node) = self.arena.nodes.get_mut(cap)
            && let WidgetPayload::TextArea {
                signal,
                cursor,
                selection,
                is_selecting,
                text_block,
                scroll_y,
                ..
            } = &mut node.payload
            && *is_selecting
        {
            let val = signal.try_get().unwrap_or_default();
            let rel_x = x - (node.bounds.x + 10.0);
            let rel_y = y - (node.bounds.y + 10.0) + *scroll_y;
            let anchor = snap_char_boundary(&val, selection.map(|s| s.0).unwrap_or(*cursor));
            let idx = text_block.global_byte_at_point(rel_x, rel_y);
            let new_cursor = snap_char_boundary(&val, idx);
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

        let mut redraw = false;

        if let Some(cm) = &mut self.context_menu {
            let next_row = if cm.menu_rect.contains(x, y) {
                let mut found = None;
                let mut cur_y = cm.menu_rect.y + 4.0;
                for (i, item) in cm.labels.iter().enumerate() {
                    let h = if item.is_some() { 28.0 } else { 9.0 };
                    if item.is_some() && y >= cur_y && y < cur_y + h {
                        found = Some(i);
                        break;
                    }
                    cur_y += h;
                }
                found
            } else {
                None
            };
            if cm.hovered_idx != next_row {
                cm.hovered_idx = next_row;
                redraw = true;
            }
            return redraw;
        }

        if !self.open_dropdowns.is_empty() {
            let mut next_dd_row = None;
            for &(dd_id, menu_rect, count) in self.open_dropdowns.iter().rev() {
                if menu_rect.contains(x, y) {
                    let idx = ((y - (menu_rect.y + 4.0)) / 30.0).floor().max(0.0) as usize;
                    next_dd_row = Some((dd_id, idx.min(count.saturating_sub(1))));
                    break;
                }
            }
            if next_dd_row != self.hovered_dropdown_row {
                self.hovered_dropdown_row = next_dd_row;
                redraw = true;
            }
            if next_dd_row.is_some() {
                redraw |= self.clear_tooltip();
                return redraw;
            }
        } else if self.hovered_dropdown_row.take().is_some() {
            redraw = true;
        }

        for &vid in &self.arena.virtual_list_ids {
            let screen_bounds = self
                .arena
                .root
                .and_then(|root| widget_screen_bounds(root, &self.arena.nodes, vid, (0.0, 0.0)));
            if let (Some(screen), Some(node)) = (screen_bounds, self.arena.nodes.get(vid))
                && let WidgetPayload::VirtualizedList { state } = &node.payload
            {
                let mut st = state.borrow_mut();
                let next_row = if screen.contains(x, y) {
                    let stride = st.item_height + st.gap;
                    let idx = ((y - screen.y + st.scroll_y.value) / stride).floor() as isize;
                    if idx >= 0 && (idx as usize) < st.item_count {
                        Some(idx as usize)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if st.hovered_row != next_row {
                    st.hovered_row = next_row;
                    redraw = true;
                }
            }
        }

        let next = self.hit_test(x, y);
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

        let next_tt = self
            .arena
            .root
            .and_then(|root| find_tooltip_node(root, &self.arena.nodes, x, y, (0.0, 0.0)));

        let next_tt_id = next_tt.map(|(wid, _)| wid);
        if next_tt_id != self.tooltip_id {
            redraw |= self.clear_tooltip();
            if let Some((wid, anchor)) = next_tt {
                self.tooltip_id = Some(wid);
                self.tooltip_anchor = anchor;
                self.tooltip_show_at = Some(Instant::now() + TOOLTIP_HOVER_DELAY);
            }
        }

        redraw
    }

    fn on_secondary_down(&mut self, x: f32, y: f32) -> bool {
        let mut redraw = self.clear_tooltip();

        for (dd_id, _, _) in self.open_dropdowns.drain(..) {
            if let Some(node) = self.arena.nodes.get_mut(dd_id)
                && let WidgetPayload::Dropdown { is_open, .. } = &mut node.payload
            {
                *is_open = false;
                redraw = true;
            }
        }

        let hit = self
            .arena
            .root
            .and_then(|root| find_context_menu_node(root, &self.arena.nodes, x, y, (0.0, 0.0)));

        if let Some(owner) = hit {
            self.context_menu = Some(ContextMenuState {
                owner,
                anchor: (x, y),
                menu_rect: Rect::default(),
                hovered_idx: None,
                labels: Vec::new(),
            });
            return true;
        }

        if self.context_menu.take().is_some() {
            return true;
        }
        redraw
    }

    fn on_pointer_down(&mut self, x: f32, y: f32) -> (bool, WindowCommand) {
        let mut redraw = self.clear_tooltip();

        if let Some(cm) = self.context_menu.take() {
            if cm.menu_rect.contains(x, y)
                && let Some(idx) = cm.hovered_idx
                && let Some(node) = self.arena.nodes.get_mut(cm.owner)
                && let Some(item) = node.context_menu.get_mut(idx)
                && let Some(cb) = &mut item.on_select
            {
                cb();
            }
            return (true, WindowCommand::None);
        }

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
                    self.hovered_dropdown_row = None;
                    return (true, WindowCommand::None);
                }
            }
            for (dd_id, _, _) in overlays {
                if let Some(node) = self.arena.nodes.get_mut(dd_id)
                    && let WidgetPayload::Dropdown { is_open, .. } = &mut node.payload
                {
                    *is_open = false;
                }
            }
            self.hovered_dropdown_row = None;
            redraw = true;
        }

        let Some(hit) = self.hit_test(x, y) else {
            let focus_changed = self.arena.focus.set_focus(None);
            self.reset_caret_blink();
            if self.is_drag_region(x, y) {
                return (redraw, WindowCommand::None);
            }
            return (redraw || focus_changed, WindowCommand::None);
        };

        self.arena.focus.set_focus(Some(hit));
        let mut clicked = false;
        let mut capture_target = false;
        let hit_screen = self
            .arena
            .root
            .and_then(|root| widget_screen_bounds(root, &self.arena.nodes, hit, (0.0, 0.0)))
            .unwrap_or_default();

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
                WidgetPayload::Container {
                    on_click: Some(cb), ..
                } => {
                    cb();
                    clicked = true;
                }
                WidgetPayload::Image {
                    on_click: Some(cb), ..
                } => {
                    cb();
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
                WidgetPayload::SplitPane { .. } => {
                    capture_target = true;
                    clicked = true;
                }
                WidgetPayload::VirtualizedList { state } => {
                    let mut st = state.borrow_mut();
                    let stride = st.item_height + st.gap;
                    let idx = ((y - hit_screen.y + st.scroll_y.value) / stride).floor() as isize;
                    if idx >= 0 && (idx as usize) < st.item_count {
                        let i = idx as usize;
                        if let Some(sel) = st.selected {
                            sel.set(i);
                        }
                        if let Some(cb) = &mut st.on_select {
                            cb(i);
                        }
                        clicked = true;
                    }
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
                    let val = signal.try_get().unwrap_or_default();
                    let rel_x = x - (node.bounds.x + 12.0);
                    let char_idx =
                        snap_char_boundary(&val, find_char_index_at_x(char_offsets, rel_x));
                    *cursor = char_idx;
                    *selection = None;
                    *is_selecting = true;
                    capture_target = true;
                    clicked = true;
                }
                WidgetPayload::TextArea {
                    signal,
                    cursor,
                    selection,
                    is_selecting,
                    text_block,
                    scroll_y,
                    ..
                } => {
                    let val = signal.try_get().unwrap_or_default();
                    let rel_x = x - (node.bounds.x + 10.0);
                    let rel_y = y - (node.bounds.y + 10.0) + *scroll_y;
                    let idx = text_block.global_byte_at_point(rel_x, rel_y);
                    *cursor = snap_char_boundary(&val, idx);
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
        (redraw || clicked, WindowCommand::None)
    }

    fn on_pointer_up(&mut self) -> bool {
        let mut was = self.captured.is_some();
        if let Some(cap) = self.captured.take()
            && let Some(node) = self.arena.nodes.get_mut(cap)
            && matches!(
                node.payload,
                WidgetPayload::TextInput { .. } | WidgetPayload::TextArea { .. }
            )
        {
            match &mut node.payload {
                WidgetPayload::TextInput { is_selecting, .. }
                | WidgetPayload::TextArea { is_selecting, .. } => *is_selecting = false,
                _ => {}
            }
            was = true;
        }
        was
    }

    fn on_window_focus(&mut self, focused: bool) -> bool {
        if !focused {
            self.captured = None;
            self.hovered = None;
            self.clear_tooltip();
            self.caret_visible = false;
            self.next_blink_at = None;
            return true;
        }
        self.reset_caret_blink();
        true
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) {
        let frame_start = FrameProfiler::begin_timer();
        for handle in self.arena.pending_texture_releases.drain(..) {
            ctx.gpu_mut().release_texture(handle);
        }
        self.profiler.record_cadence(ctx.frame_dt);

        let mode = self
            .theme_signal
            .and_then(|s| s.try_get())
            .unwrap_or(ThemeMode::Light);
        let theme = MacTheme::from_mode(mode);

        // Stage 1: Drain per-arena reactive dirty notifications
        let dirty_batch: Vec<(WidgetId, DirtyFlags)> =
            std::mem::take(&mut *self.arena.dirty_queue.borrow_mut());
        let mut foreach_dirty: Vec<WidgetId> = Vec::new();
        let mut show_dirty: Vec<WidgetId> = Vec::new();
        for (id, flags) in dirty_batch {
            let Some(node) = self.arena.nodes.get(id) else { continue; };
            let lnode = node.layout_node;
            let is_foreach = matches!(node.payload, WidgetPayload::ForEach { .. });
            let is_show = matches!(node.payload, WidgetPayload::Show { .. });

            if flags.layout {
                self.arena.layout.mark_dirty(lnode);
                let mut cur = id;
                while let Some(n) = self.arena.nodes.get(cur) {
                    if n.is_relayout_boundary || n.parent.is_none() {
                        break;
                    }
                    if let Some(p) = n.parent {
                        cur = p;
                    } else {
                        break;
                    }
                }
                if Some(cur) == self.arena.root {
                    self.arena.needs_full_layout = true;
                } else if let Some(n) = self.arena.nodes.get_mut(cur)
                    && !n.dirty_layout_queued
                {
                    n.dirty_layout_queued = true;
                    self.arena.dirty_layout_roots.push(cur);
                }
            }

            if is_foreach && (flags.state || flags.layout) {
                foreach_dirty.push(id);
            }
            if is_show && (flags.state || flags.layout) {
                show_dirty.push(id);
            }
            let node = self.arena.nodes.get_mut(id).unwrap();
                match &mut node.payload {
                    WidgetPayload::Text {
                        block,
                        pending: Some(cell),
                        ..
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
                        let text_val = signal.try_get().unwrap_or_default();
                        if *last_synced_text != text_val {
                            *last_synced_text = text_val.clone();
                            *cursor = snap_char_boundary(&text_val, *cursor);
                            *selection = normalize_selection(&text_val, *selection);
                            text_block.set_text(ctx.text_engine(), &text_val);
                            text_block.collect_char_offsets(char_offsets);
                        }
                    }
                    WidgetPayload::TextArea {
                        signal,
                        cursor,
                        selection,
                        text_block,
                        last_synced_text,
                        ..
                    } => {
                        let text_val = signal.try_get().unwrap_or_default();
                        if *last_synced_text != text_val {
                            *last_synced_text = text_val.clone();
                            *cursor = snap_char_boundary(&text_val, *cursor);
                            *selection = normalize_selection(&text_val, *selection);
                            text_block.set_text(ctx.text_engine(), &text_val);
                        }
                    }
                    _ => {}
                }
        }

        for id in show_dirty {
            let (cond, active, mut when_true, mut when_false) =
                if let Some(node) = self.arena.nodes.get_mut(id) {
                    if let WidgetPayload::Show {
                        condition,
                        active,
                        when_true,
                        when_false,
                    } = &mut node.payload
                    {
                        (*condition, *active, when_true.take(), when_false.take())
                    } else {
                        continue;
                    }
                } else {
                    continue;
                };
            let next = cond.try_get().unwrap_or(false);
            if next != active {
                let old_children: Vec<WidgetId> = self
                    .arena
                    .nodes
                    .get(id)
                    .map(|n| n.children.clone())
                    .unwrap_or_default();
                for child in old_children {
                    self.arena.unmount_subtree(child);
                }
                let new_child = if next {
                    when_true
                        .as_mut()
                        .map(|f| self.arena.mount_node(f(), ctx.text_engine()))
                } else {
                    when_false
                        .as_mut()
                        .map(|f| self.arena.mount_node(f(), ctx.text_engine()))
                };
                let children: Vec<WidgetId> = new_child.into_iter().collect();
                for &child in &children {
                    if let Some(node) = self.arena.nodes.get_mut(child) {
                        node.parent = Some(id);
                    }
                }
                let child_lnodes: Vec<LayoutNode> = children
                    .iter()
                    .filter_map(|cid| self.arena.nodes.get(*cid).map(|n| n.layout_node))
                    .collect();
                if let Some(node) = self.arena.nodes.get_mut(id) {
                    node.children = children;
                    let lnode = node.layout_node;
                    if let WidgetPayload::Show {
                        active,
                        when_true: wt,
                        when_false: wf,
                        ..
                    } = &mut node.payload
                    {
                        *active = next;
                        *wt = when_true;
                        *wf = when_false;
                    }
                    self.arena.layout.set_children(lnode, &child_lnodes);
                    self.arena.layout.mark_dirty(lnode);
                }
                self.arena.needs_full_layout = true;
            } else if let Some(node) = self.arena.nodes.get_mut(id)
                && let WidgetPayload::Show {
                    when_true: wt,
                    when_false: wf,
                    ..
                } = &mut node.payload
            {
                *wt = when_true;
                *wf = when_false;
            }
        }

        for id in foreach_dirty {
            let reconcile_fn = if let Some(node) = self.arena.nodes.get_mut(id) {
                if let WidgetPayload::ForEach { reconcile } = &mut node.payload {
                    Some(std::mem::replace(reconcile, Box::new(|_, _, _| {})))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(mut reconcile_fn) = reconcile_fn {
                reconcile_fn(&mut self.arena, ctx.text_engine(), id);
                if let Some(node) = self.arena.nodes.get_mut(id)
                    && let WidgetPayload::ForEach { reconcile } = &mut node.payload
                {
                    *reconcile = reconcile_fn;
                }
                self.arena.needs_full_layout = true;
            }
        }

        let mut split_syncs = Vec::new();
        for &sp_id in &self.arena.split_ids {
            if let Some(node) = self.arena.nodes.get(sp_id)
                && let WidgetPayload::SplitPane {
                    direction,
                    ratio,
                    min_ratio,
                    max_ratio,
                    first_id,
                    ..
                } = &node.payload
            {
                let r = ratio.try_get().unwrap_or(0.3).clamp(*min_ratio, *max_ratio);
                split_syncs.push((*first_id, *direction, r));
            }
        }
        for (first_id, dir, r) in split_syncs {
            if let Some(first_node) = self.arena.nodes.get_mut(first_id) {
                let target_len = Length::Percent(r);
                let changed = match dir {
                    SplitDirection::Horizontal => first_node.layout_style.width != target_len,
                    SplitDirection::Vertical => first_node.layout_style.height != target_len,
                };
                if changed {
                    match dir {
                        SplitDirection::Horizontal => first_node.layout_style.width = target_len,
                        SplitDirection::Vertical => first_node.layout_style.height = target_len,
                    }
                    self.arena
                        .layout
                        .set_style(first_node.layout_node, first_node.layout_style);
                    self.arena.needs_full_layout = true;
                }
            }
        }

        if (ctx.width, ctx.height) != self.last_size {
            self.last_size = (ctx.width, ctx.height);
            self.arena.needs_full_layout = true;
        }

        // Stage 2: Incremental Taffy Layout + ScrollView content_height
        let layout_start = FrameProfiler::begin_timer();
        let mut did_layout = false;
        if (self.arena.needs_full_layout || self.arena.dirty_layout_roots.len() > 32)
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
                        WidgetPayload::TextArea { text_block, .. } => {
                            text_block.measure(text_engine, avail_w.map(|w| (w - 20.0).max(1.0)))
                        }
                        WidgetPayload::Button {
                            label,
                            label_empty,
                            icon_state,
                            icon_size,
                            ..
                        } => {
                            let (w, _) = label.measure(text_engine, None);
                            let has_icon = icon_state.borrow().handle.is_some()
                                || icon_state.borrow().image.is_some();
                            if has_icon && *label_empty {
                                (30.0, 30.0)
                            } else if has_icon {
                                (w + 28.0 + *icon_size, 30.0)
                            } else {
                                (w + 28.0, 30.0)
                            }
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

            self.arena.needs_full_layout = false;
            for &id in &self.arena.dirty_layout_roots {
                if let Some(n) = self.arena.nodes.get_mut(id) {
                    n.dirty_layout_queued = false;
                }
            }
            self.arena.dirty_layout_roots.clear();
            did_layout = true;
        } else if !self.arena.dirty_layout_roots.is_empty() {
            let roots: Vec<WidgetId> = std::mem::take(&mut self.arena.dirty_layout_roots);
            for boundary_id in roots {
                if let Some(n) = self.arena.nodes.get_mut(boundary_id) {
                    n.dirty_layout_queued = false;
                }
                let Some(boundary_node) = self.arena.nodes.get(boundary_id) else {
                    continue;
                };
                let boundary_lnode = boundary_node.layout_node;
                let bw = boundary_node.bounds.width.max(1.0);
                let bh = boundary_node.bounds.height.max(1.0);
                let origin_x = boundary_node.bounds.x;
                let origin_y = boundary_node.bounds.y;

                let nodes = &mut self.arena.nodes;
                let lmap = &self.arena.layout_to_widget;
                let text_engine = ctx.text_engine();

                self.arena.layout.compute_subtree_with_measure(
                    boundary_lnode,
                    bw,
                    bh,
                    |lnode, _, _, avail_w, _| {
                        let Some(&wid) = lmap.get(&lnode) else {
                            return (0.0, 0.0);
                        };
                        let Some(node) = nodes.get_mut(wid) else {
                            return (0.0, 0.0);
                        };
                        match &mut node.payload {
                            WidgetPayload::Text { block, .. } => {
                                block.measure(text_engine, avail_w)
                            }
                            WidgetPayload::TextArea { text_block, .. } => text_block
                                .measure(text_engine, avail_w.map(|w| (w - 20.0).max(1.0))),
                            WidgetPayload::Button {
                                label,
                                label_empty,
                                icon_state,
                                icon_size,
                                ..
                            } => {
                                let (w, _) = label.measure(text_engine, None);
                                let has_icon = icon_state.borrow().handle.is_some()
                                    || icon_state.borrow().image.is_some();
                                if has_icon && *label_empty {
                                    (30.0, 30.0)
                                } else if has_icon {
                                    (w + 28.0 + *icon_size, 30.0)
                                } else {
                                    (w + 28.0, 30.0)
                                }
                            }
                            _ => (0.0, 0.0),
                        }
                    },
                );

                let slice = self
                    .arena
                    .layout
                    .collect_logical_rects_into(boundary_lnode, origin_x, origin_y);
                self.arena.rect_buf.clear();
                self.arena.rect_buf.extend_from_slice(slice);
                for i in 0..self.arena.rect_buf.len() {
                    let (lnode, rect) = self.arena.rect_buf[i];
                    if let Some(&wid) = self.arena.layout_to_widget.get(&lnode)
                        && let Some(node) = self.arena.nodes.get_mut(wid)
                    {
                        node.bounds = rect;
                    }
                }
            }
            did_layout = true;
        }
        let layout_ms = layout_start.elapsed().as_secs_f32() * 1000.0;

        if let Some(cm) = &mut self.context_menu
            && cm.labels.is_empty()
            && let Some(owner_node) = self.arena.nodes.get(cm.owner)
        {
            let te = ctx.text_engine();
            for item in &owner_node.context_menu {
                if let Some(lbl) = &item.label {
                    let normal = TextBlock::new(
                        te,
                        lbl,
                        TextStyle {
                            font_size: 13.0,
                            line_height: 18.0,
                            color: theme.text_primary,
                            weight: FontWeight::Medium,
                        },
                    );
                    let selected = TextBlock::new(
                        te,
                        lbl,
                        TextStyle {
                            font_size: 13.0,
                            line_height: 18.0,
                            color: theme.text_on_accent,
                            weight: FontWeight::Medium,
                        },
                    );
                    cm.labels.push(Some((normal, selected)));
                } else {
                    cm.labels.push(None);
                }
            }
        }

        if self.tooltip_visible
            && self.tooltip_cached_id != self.tooltip_id
            && let Some(tid) = self.tooltip_id
            && let Some(tt_text) = self.arena.nodes.get(tid).and_then(|n| n.tooltip.as_deref())
        {
            let te = ctx.text_engine();
            let style = TextStyle {
                font_size: 11.5,
                line_height: 15.0,
                color: theme.text_primary,
                weight: FontWeight::Medium,
            };
            self.tooltip_block = Some(TextBlock::new(te, tt_text, style));
            self.tooltip_cached_id = Some(tid);
        }

        // Stage 3: Hierarchical Paint + Overlays + Lazy SemanticTree
        let paint_start = FrameProfiler::begin_timer();
        let mut overlay_records = Vec::new();
        self.open_dropdowns.clear();

        if let Some(draw) = &mut self.path_draw_fn {
            draw(ctx);
        }

        if let Some(root) = self.arena.root {
            let scale = ctx.scale;
            let time_secs = self.start_instant.elapsed().as_secs_f32();
            let mut a11y_buf = if self.a11y_active {
                Some(Vec::new())
            } else {
                None
            };
            let mut selection_scratch = Vec::new();
            let mut pctx = PaintCtx {
                ctx,
                theme: &theme,
                scale,
                time_secs,
                caret_visible: self.caret_visible,
                hovered: self.hovered,
                focused: self.arena.focus.focused,
                overlays: &mut overlay_records,
                a11y_records: a11y_buf.as_mut(),
                selection_scratch: &mut selection_scratch,
                decoded_tex_cache: &mut self.arena.decoded_tex_cache,
            };

            paint_node(root, &self.arena.nodes, &mut pctx, (0.0, 0.0));

            if let Some(records) = a11y_buf {
                self.cached_semantic_tree = Some(SemanticTree {
                    root,
                    focus: self.arena.focus.focused,
                    nodes: records,
                });
            }

            // 3A. Open Dropdown Overlays
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
                    shadow_color: theme.shadow_elevated,
                    shadow_offset: [0.0, 8.0 * scale],
                    shadow_blur: 20.0 * scale,
                    ..Default::default()
                });
                ctx.push_glass(GlassQuad {
                    rect: snap_rect_to_pixels(menu_rect, scale),
                    radii: CornerRadii::all(10.0).scale(scale),
                    tint: theme.popover_glass_tint,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                    frost: 0.42,
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
                        let is_hov = self.hovered_dropdown_row == Some((ov.source_id, i));
                        if is_sel || is_hov {
                            ctx.push_sdf(SdfQuad {
                                rect: snap_rect_to_pixels(row_rect, scale),
                                radii: CornerRadii::all(6.0).scale(scale),
                                fill: if is_sel {
                                    theme.accent
                                } else {
                                    theme.control_hover
                                },
                                ..Default::default()
                            });
                        }
                        let block = if is_sel {
                            &selected_labels[i]
                        } else {
                            &labels[i]
                        };
                        ctx.push_text_colored(
                            block,
                            row_rect.x + 10.0,
                            row_rect.y + 6.0,
                            if is_sel {
                                theme.text_on_accent
                            } else {
                                theme.text_primary
                            },
                        );
                    }
                }
            }

            // 3B. Active Right-Click ContextMenu Overlay
            if let Some(cm) = &mut self.context_menu
                && !cm.labels.is_empty()
            {
                let menu_w = 196.0_f32.min(ctx.width - 16.0);
                let menu_h = cm
                    .labels
                    .iter()
                    .map(|l| if l.is_some() { 28.0 } else { 9.0 })
                    .sum::<f32>()
                    + 8.0;

                let menu_x = if cm.anchor.0 + menu_w <= ctx.width - 8.0 {
                    cm.anchor.0
                } else {
                    (cm.anchor.0 - menu_w).max(8.0)
                };
                let menu_y = if cm.anchor.1 + menu_h <= ctx.height - 8.0 {
                    cm.anchor.1
                } else {
                    (cm.anchor.1 - menu_h).max(8.0)
                };

                let menu_rect = Rect::new(menu_x, menu_y, menu_w, menu_h);
                cm.menu_rect = menu_rect;

                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(menu_rect, scale),
                    radii: CornerRadii::all(10.0).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: theme.shadow_elevated,
                    shadow_offset: [0.0, 10.0 * scale],
                    shadow_blur: 22.0 * scale,
                    ..Default::default()
                });
                ctx.push_glass(GlassQuad {
                    rect: snap_rect_to_pixels(menu_rect, scale),
                    radii: CornerRadii::all(10.0).scale(scale),
                    tint: theme.popover_glass_tint,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                    frost: 0.45,
                    specular: 0.85,
                    ..Default::default()
                });

                let mut cur_y = menu_y + 4.0;
                for (i, item_opt) in cm.labels.iter().enumerate() {
                    if let Some((normal, selected)) = item_opt {
                        let row_rect = Rect::new(menu_x + 4.0, cur_y, menu_w - 8.0, 28.0);
                        let is_hov = cm.hovered_idx == Some(i);
                        if is_hov {
                            ctx.push_sdf(SdfQuad {
                                rect: snap_rect_to_pixels(row_rect, scale),
                                radii: CornerRadii::all(6.0).scale(scale),
                                fill: theme.accent,
                                ..Default::default()
                            });
                        }
                        let blk = if is_hov { selected } else { normal };
                        ctx.push_text_colored(
                            blk,
                            row_rect.x + 10.0,
                            row_rect.y + 5.0,
                            if is_hov {
                                theme.text_on_accent
                            } else {
                                theme.text_primary
                            },
                        );
                        cur_y += 28.0;
                    } else {
                        ctx.push_sdf(SdfQuad {
                            rect: Rect::new(
                                ((menu_x + 12.0) * scale).round(),
                                ((cur_y + 4.0) * scale).round(),
                                ((menu_w - 24.0) * scale).round(),
                                1.0,
                            ),
                            fill: theme.hairline,
                            ..Default::default()
                        });
                        cur_y += 9.0;
                    }
                }
            }

            // 3C. Hover-Delayed Tooltip Pill
            if self.tooltip_visible
                && let Some(tb) = &mut self.tooltip_block
            {
                let (tw, th) = tb.measure(ctx.text_engine(), Some(280.0));
                let box_w = tw + 18.0;
                let box_h = th + 10.0;
                let tx = (self.tooltip_anchor.x + (self.tooltip_anchor.width - box_w) * 0.5)
                    .clamp(8.0, (ctx.width - box_w - 8.0).max(8.0));
                let below_y = self.tooltip_anchor.y + self.tooltip_anchor.height + 6.0;
                let ty = if below_y + box_h <= ctx.height - 8.0 {
                    below_y
                } else {
                    (self.tooltip_anchor.y - box_h - 6.0).max(8.0)
                };

                let tt_rect = Rect::new(tx, ty, box_w, box_h);
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(tt_rect, scale),
                    radii: CornerRadii::all(6.0).scale(scale),
                    fill: theme.panel_bg,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                    shadow_color: theme.shadow_soft,
                    shadow_offset: [0.0, 4.0 * scale],
                    shadow_blur: 12.0 * scale,
                    ..Default::default()
                });
                ctx.push_text_colored(tb, tt_rect.x + 9.0, tt_rect.y + 5.0, theme.text_primary);
            }
        }

        if self.show_fps {
            let m = self.profiler.metrics;
            let should_refresh = self.fps_block.is_none()
                || self.last_fps_update.elapsed() >= Duration::from_millis(150);
            if should_refresh {
                self.last_fps_update = Instant::now();
                let label = if self.was_animating && m.effective_fps > 10.0 {
                    format!(
                        "{:.0} FPS  •  {:.2} ms (L:{:.2} P:{:.2})",
                        m.effective_fps, m.frame_ms, m.layout_ms, m.paint_ms
                    )
                } else {
                    format!(
                        "Idle  •  {:.2} ms (L:{:.2} P:{:.2})",
                        m.frame_ms, m.layout_ms, m.paint_ms
                    )
                };
                let te = ctx.text_engine();
                if let Some(blk) = &mut self.fps_block {
                    blk.set_text(te, &label);
                } else {
                    self.fps_block = Some(TextBlock::new(
                        te,
                        label,
                        TextStyle {
                            font_size: 11.0,
                            line_height: 14.0,
                            color: theme.text_primary,
                            weight: FontWeight::Medium,
                        },
                    ));
                }
            }

            if let Some(blk) = &mut self.fps_block {
                let (tw, _) = blk.measure(ctx.text_engine(), None);
                let pill_w = tw + 16.0;
                let pill_h = 22.0;
                let pill_rect = Rect::new(
                    (ctx.width - pill_w - 14.0).max(8.0),
                    (ctx.height - pill_h - 10.0).max(8.0),
                    pill_w,
                    pill_h,
                );
                let scale = ctx.scale;
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(pill_rect, scale),
                    radii: CornerRadii::all(6.0).scale(scale),
                    fill: theme.panel_bg,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                    ..Default::default()
                });
                ctx.push_text_colored(
                    blk,
                    pill_rect.x + 8.0,
                    pill_rect.y + 4.0,
                    theme.text_primary,
                );
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

    if let Some(a11y) = pctx.a11y_records.as_deref_mut() {
        let (role, label, value, numeric_value, toggled, focusable, clickable) = match &node.payload
        {
            WidgetPayload::Text { block, .. } => (
                A11yRole::Label,
                node.tooltip.clone(),
                Some(format!("{:.0}", block.style().font_size)),
                None,
                None,
                false,
                false,
            ),
            WidgetPayload::Button { .. } => (
                A11yRole::Button,
                node.tooltip.clone(),
                None,
                None,
                None,
                true,
                true,
            ),
            WidgetPayload::Switch { signal } => (
                A11yRole::Switch,
                node.tooltip.clone().or_else(|| Some("Switch".into())),
                None,
                None,
                signal.try_get(),
                true,
                true,
            ),
            WidgetPayload::Slider { signal, .. } => (
                A11yRole::Slider,
                node.tooltip.clone().or_else(|| Some("Slider".into())),
                None,
                signal.try_get().map(|v| v as f64),
                None,
                true,
                false,
            ),
            WidgetPayload::TextInput { signal, .. } => (
                A11yRole::TextInput,
                node.tooltip.clone().or_else(|| Some("Text Input".into())),
                signal.try_get(),
                None,
                None,
                true,
                true,
            ),
            WidgetPayload::TextArea { signal, .. } => (
                A11yRole::TextInput,
                node.tooltip.clone().or_else(|| Some("Text Area".into())),
                signal.try_get(),
                None,
                None,
                true,
                true,
            ),
            _ => (A11yRole::Container, None, None, None, None, false, false),
        };
        a11y.push(A11yNodeRecord {
            id: wid,
            role,
            label,
            value,
            numeric_value,
            toggled,
            focusable,
            clickable,
            bounds: bounds.scale(scale),
            children: node.children.clone(),
        });
    }

    match &node.payload {
        WidgetPayload::Container {
            material,
            radius,
            hover_fill,
            ..
        } => {
            render_material(pctx.ctx, bounds, material.get(), radius.get(), scale);
            if is_hovered && let Some(fill) = *hover_fill {
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(radius.get()).scale(scale),
                    fill,
                    ..Default::default()
                });
            }
            for &child in &node.children {
                paint_node(child, nodes, pctx, offset);
            }
        }
        WidgetPayload::Switcher { .. }
        | WidgetPayload::Show { .. }
        | WidgetPayload::ForEach { .. } => {
            for &child in &node.children {
                paint_node(child, nodes, pctx, offset);
            }
        }
        WidgetPayload::Text { block, tone, .. } => {
            let color = tone.resolve(theme, block.style().color);
            pctx.ctx.push_text_colored(block, bounds.x, bounds.y, color);
        }
        WidgetPayload::Image {
            state,
            radius,
            tint,
            shadow,
            fit,
            ..
        } => {
            if let Some(shadow) = shadow {
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(*radius).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: shadow.color,
                    shadow_offset: [shadow.offset[0] * scale, shadow.offset[1] * scale],
                    shadow_blur: shadow.blur * scale,
                    ..Default::default()
                });
            }
            let mut st = state.borrow_mut();
            if st.handle.is_none() {
                if let Some(key) = st.source_key {
                    if let Some((handle, refs)) = pctx.decoded_tex_cache.get_mut(&key) {
                        *refs += 1;
                        st.handle = Some(*handle);
                        st.image = None;
                    } else if let Some(img) = st.image.take() {
                        let handle = pctx.ctx.gpu_mut().upload_image(img.as_ref());
                        pctx.decoded_tex_cache.insert(key, (handle, 1));
                        st.handle = Some(handle);
                    }
                } else if let Some(img) = st.image.take() {
                    st.handle = Some(pctx.ctx.gpu_mut().upload_image(img.as_ref()));
                }
            }
            if let Some(handle) = st.handle {
                let uv = match fit {
                    ImageFit::Cover => {
                        cover_uv_rect(st.width, st.height, bounds.width, bounds.height)
                    }
                    ImageFit::Contain | ImageFit::Stretch => [0.0, 0.0, 1.0, 1.0],
                };
                pctx.ctx.push_image(ImageQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    uv_rect: uv,
                    radii: CornerRadii::all(*radius).scale(scale),
                    tint: *tint,
                    texture: handle,
                });
            }
        }
        WidgetPayload::Icon {
            state,
            tone,
            color,
            size,
        } => {
            let mut st = state.borrow_mut();
            if st.handle.is_none()
                && let Some(img) = st.image.take()
            {
                st.handle = Some(pctx.ctx.gpu_mut().upload_image(&img));
            }
            if let Some(handle) = st.handle {
                let tint = tone.resolve(theme, *color);
                pctx.ctx.push_image(ImageQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    uv_rect: [0.0, 0.0, 1.0, 1.0],
                    radii: CornerRadii::all(0.0),
                    tint,
                    texture: handle,
                });
            }
            let _ = size;
        }
        WidgetPayload::ProgressBar { signal, color } => {
            let v = signal.try_get().unwrap_or(0.0).clamp(0.0, 1.0);
            let track = snap_rect_to_pixels(bounds, scale);
            pctx.ctx.push_sdf(SdfQuad {
                rect: track,
                radii: CornerRadii::all(bounds.height * 0.5).scale(scale),
                fill: theme.hairline,
                ..Default::default()
            });
            if v > 0.0 {
                let fill_rect = Rect::new(bounds.x, bounds.y, bounds.width * v, bounds.height);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(fill_rect, scale),
                    radii: CornerRadii::all(bounds.height * 0.5).scale(scale),
                    fill: color.unwrap_or(theme.accent),
                    ..Default::default()
                });
            }
        }
        WidgetPayload::Button {
            label,
            label_empty,
            accent,
            variant,
            icon_state,
            icon_size,
            pill,
            ..
        } => {
            let r = if *pill { bounds.height * 0.5 } else { 7.0 };
            let is_primary = *variant == ButtonVariant::Primary || *accent;
            let is_glass = *variant == ButtonVariant::Glass;
            let is_ghost = *variant == ButtonVariant::Ghost;
            if is_glass {
                pctx.ctx.push_glass(GlassQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(r).scale(scale),
                    tint: if is_hovered {
                        Color::from_rgba8(255, 255, 255, 34)
                    } else {
                        Color::from_rgba8(255, 255, 255, 24)
                    },
                    saturation: 1.0,
                    frost: 0.22,
                    liquid_ripple: 0.0,
                    specular: 0.8,
                    rim_angle: -1.309,
                    sheen_progress: 0.0,
                    conic_strength: 0.0,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                });
            } else {
                let fill = if is_primary {
                    theme.accent
                } else if is_ghost {
                    if is_hovered {
                        theme.control_hover
                    } else {
                        Color::TRANSPARENT
                    }
                } else if is_hovered {
                    theme.control_hover
                } else {
                    theme.panel_bg
                };
                let border_color = if is_primary || is_ghost {
                    Color::TRANSPARENT
                } else {
                    theme.hairline
                };
                let border_width = if is_primary || is_ghost {
                    0.0
                } else {
                    1.0 * scale
                };
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(r).scale(scale),
                    fill,
                    border_color,
                    border_width,
                    shadow_color: if is_ghost {
                        Color::TRANSPARENT
                    } else {
                        theme.shadow_soft
                    },
                    shadow_offset: [0.0, 1.0 * scale],
                    shadow_blur: if is_ghost { 0.0 } else { 3.0 * scale },
                    ..Default::default()
                });
            }

            let mut st = icon_state.borrow_mut();
            if st.handle.is_none()
                && let Some(img) = st.image.take()
            {
                st.handle = Some(pctx.ctx.gpu_mut().upload_image(&img));
            }
            let has_icon = st.handle.is_some();
            let text_color = if is_primary {
                theme.text_on_accent
            } else {
                theme.text_primary
            };
            let icon_color = text_color;
            let mut text_x = bounds.x + 14.0;
            if let Some(handle) = st.handle {
                let ix = if *label_empty {
                    bounds.x + (bounds.width - *icon_size) * 0.5
                } else {
                    bounds.x + 10.0
                };
                let iy = bounds.y + (bounds.height - *icon_size) * 0.5;
                pctx.ctx.push_image(ImageQuad {
                    rect: snap_rect_to_pixels(Rect::new(ix, iy, *icon_size, *icon_size), scale),
                    uv_rect: [0.0, 0.0, 1.0, 1.0],
                    radii: CornerRadii::all(0.0),
                    tint: icon_color,
                    texture: handle,
                });
                text_x = ix + *icon_size + 8.0;
            }
            if !*label_empty {
                pctx.ctx
                    .push_text_colored(label, text_x, bounds.y + 6.0, text_color);
            }
            let _ = has_icon;
        }
        WidgetPayload::Switch { signal } => {
            let on = signal.try_get().unwrap_or(false);
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
                border_color: theme.hairline,
                border_width: 1.0 * scale,
                shadow_color: theme.shadow_soft,
                shadow_offset: [0.0, 1.0 * scale],
                shadow_blur: 3.0 * scale,
                ..Default::default()
            });
        }
        WidgetPayload::Slider { signal, min, max } => {
            let val = signal.try_get().unwrap_or(*min);
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
                border_width: 1.0 * scale,
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
            let val = signal.try_get().unwrap_or_default();
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

            let inner_clip = Rect::new(
                bounds.x + 8.0,
                bounds.y,
                (bounds.width - 16.0).max(1.0),
                bounds.height,
            );
            pctx.ctx.push_clip(inner_clip);

            let text_x = bounds.x + 12.0;
            let text_y = bounds.y + 8.0;

            if let Some((start, end)) = *selection {
                let s = snap_char_boundary(&val, start.min(end));
                let e = snap_char_boundary(&val, start.max(end));
                if s != e {
                    let x1 = _lookup_offset_x(char_offsets, s);
                    let x2 = _lookup_offset_x(char_offsets, e);
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
                pctx.ctx
                    .push_text_colored(placeholder_block, text_x, text_y, theme.text_tertiary);
            } else {
                pctx.ctx
                    .push_text_colored(text_block, text_x, text_y, theme.text_primary);
            }

            if is_focused && pctx.caret_visible {
                let safe_cursor = snap_char_boundary(&val, *cursor);
                let caret_offset_x = _lookup_offset_x(char_offsets, safe_cursor);
                let caret_x = text_x + caret_offset_x;
                let caret_rect = Rect::new(caret_x, text_y + 1.0, 1.5, 16.0);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(caret_rect, scale),
                    fill: theme.accent,
                    ..Default::default()
                });
            }

            pctx.ctx.pop_clip();
        }
        WidgetPayload::TextArea {
            signal,
            material,
            cursor,
            selection,
            text_block,
            placeholder_block,
            scroll_y,
            ..
        } => {
            let val = signal.try_get().unwrap_or_default();
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

            let inner_clip = Rect::new(
                bounds.x + 4.0,
                bounds.y + 4.0,
                (bounds.width - 8.0).max(1.0),
                (bounds.height - 8.0).max(1.0),
            );
            pctx.ctx.push_clip(inner_clip);

            let text_x = bounds.x + 10.0;
            let text_y = bounds.y + 10.0 - *scroll_y;

            if let Some((start, end)) = *selection {
                let s = snap_char_boundary(&val, start.min(end));
                let e = snap_char_boundary(&val, start.max(end));
                text_block.collect_selection_rects(s, e, pctx.selection_scratch);
                for i in 0..pctx.selection_scratch.len() {
                    let (sx, sy, sw) = pctx.selection_scratch[i];
                    let sel_rect = Rect::new(text_x + sx, text_y + sy, sw, 18.0);
                    pctx.ctx.push_sdf(SdfQuad {
                        rect: snap_rect_to_pixels(sel_rect, scale),
                        radii: CornerRadii::all(3.0).scale(scale),
                        fill: Color::rgba(theme.accent.r, theme.accent.g, theme.accent.b, 0.28),
                        ..Default::default()
                    });
                }
            }

            if val.is_empty() {
                pctx.ctx
                    .push_text_colored(placeholder_block, text_x, text_y, theme.text_tertiary);
            } else {
                pctx.ctx
                    .push_text_colored(text_block, text_x, text_y, theme.text_primary);
            }

            if is_focused && pctx.caret_visible {
                let safe_cursor = snap_char_boundary(&val, *cursor);
                let (cy, cx) = text_block.caret_pos_for_byte(safe_cursor);
                let caret_rect = Rect::new(text_x + cx, text_y + cy + 1.0, 1.5, 16.0);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(caret_rect, scale),
                    fill: theme.accent,
                    ..Default::default()
                });
            }

            pctx.ctx.pop_clip();
        }
        WidgetPayload::Tabs {
            items,
            labels,
            label_widths,
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
                let lw = label_widths.get(i).copied().unwrap_or(0.0);
                pctx.ctx.push_text_colored(
                    label,
                    item_cx - lw * 0.5,
                    bounds.y + 6.0,
                    theme.text_primary,
                );
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
            let selected = signal
                .try_get()
                .unwrap_or(0)
                .min(items.len().saturating_sub(1));
            if let Some(lbl) = labels.get(selected) {
                pctx.ctx.push_text_colored(
                    lbl,
                    bounds.x + 12.0,
                    bounds.y + 7.0,
                    theme.text_primary,
                );
            }
            pctx.ctx.push_text_colored(
                chevron,
                bounds.x + bounds.width - 20.0,
                bounds.y + 8.0,
                theme.text_secondary,
            );

            if *is_open {
                pctx.overlays.push(OverlayRenderRecord {
                    source_id: wid,
                    trigger_bounds: bounds,
                    selected,
                    item_count: items.len(),
                });
            }
        }
        WidgetPayload::CustomGpuSurface {
            radius,
            animate_signal,
            resolution_scale,
            state,
            ..
        } => {
            let snapped = snap_rect_to_pixels(bounds, scale);
            let pw = ((snapped.width * *resolution_scale).round() as u32).max(1);
            let ph = ((snapped.height * *resolution_scale).round() as u32).max(1);
            let mut st = state.borrow_mut();

            let gpu = pctx.ctx.gpu_mut();
            if st.size != (pw, ph) || st.handle.is_none() {
                if let Some(old_handle) = st.handle.take() {
                    gpu.release_texture(old_handle);
                }
                let (view, handle) = gpu.create_render_texture(pw, ph, ATLAS_FORMAT);
                st.view = Some(view);
                st.handle = Some(handle);
                st.size = (pw, ph);
                st.rendered_once = false;
            }

            let should_render = !st.rendered_once
                || animate_signal
                    .as_ref()
                    .is_some_and(|s| s.try_get().unwrap_or(false));
            let view = st.view.clone();
            if let (Some(view), Some(handle)) = (view, st.handle) {
                if should_render {
                    let mut encoder =
                        gpu.device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Cadmium CustomGpuSurface Encoder"),
                            });
                    st.pass.render(
                        &gpu.device,
                        &gpu.queue,
                        &mut encoder,
                        &view,
                        ATLAS_FORMAT,
                        pw,
                        ph,
                        pctx.time_secs,
                    );
                    gpu.queue.submit(std::iter::once(encoder.finish()));
                    st.rendered_once = true;
                }

                pctx.ctx.push_image(ImageQuad {
                    rect: snapped,
                    uv_rect: [0.0, 0.0, 1.0, 1.0],
                    radii: CornerRadii::all(radius.get()).scale(scale),
                    tint: Color::rgba(1.0, 1.0, 1.0, 1.0),
                    texture: handle,
                });
            }
        }
        WidgetPayload::SplitPane {
            direction,
            divider_id,
            divider_color,
            ..
        } => {
            for &child in &node.children {
                paint_node(child, nodes, pctx, offset);
            }
            if let Some(div) = nodes.get(*divider_id) {
                let db = Rect::new(
                    div.bounds.x + offset.0,
                    div.bounds.y + offset.1,
                    div.bounds.width,
                    div.bounds.height,
                );
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(db, scale),
                    fill: divider_color.unwrap_or(theme.hairline),
                    ..Default::default()
                });
                if is_hovered {
                    let pill = match direction {
                        SplitDirection::Horizontal => {
                            Rect::new(db.x - 1.5, db.y + db.height * 0.5 - 18.0, 4.0, 36.0)
                        }
                        SplitDirection::Vertical => {
                            Rect::new(db.x + db.width * 0.5 - 18.0, db.y - 1.5, 36.0, 4.0)
                        }
                    };
                    pctx.ctx.push_sdf(SdfQuad {
                        rect: snap_rect_to_pixels(pill, scale),
                        radii: CornerRadii::all(2.0).scale(scale),
                        fill: theme.accent,
                        ..Default::default()
                    });
                }
            }
        }
        WidgetPayload::VirtualizedList { state } => {
            let mut st = state.borrow_mut();
            let viewport = bounds;
            let stride = st.item_height + st.gap;
            let total_h = if st.item_count == 0 {
                0.0
            } else {
                st.item_count as f32 * stride - st.gap
            };
            let scroll_val = st
                .scroll_y
                .value
                .clamp(0.0, (total_h - viewport.height).max(0.0));

            pctx.ctx.push_clip(viewport);

            if st.item_count > 0 && viewport.height > 0.0 {
                let first = (scroll_val / stride).floor().max(0.0) as usize;
                let visible_n = (viewport.height / stride).ceil() as usize + 2;
                let last = (first + visible_n).min(st.item_count);
                let selected_idx = st.selected.and_then(|s| s.try_get());
                let hovered_idx = st.hovered_row;
                let item_h = st.item_height;
                let pool_len = st.pool.len().max(1);

                for idx in first..last {
                    let slot_idx = idx % pool_len;
                    let needs_build = st.pool[slot_idx].cached_index != Some(idx);
                    if needs_build {
                        let row_data = (st.row_builder)(idx);
                        let te = pctx.ctx.text_engine();
                        let slot = &mut st.pool[slot_idx];
                        slot.cached_index = Some(idx);
                        slot.title.set_text(te, &row_data.title);
                        slot.subtitle.set_text(te, &row_data.subtitle);
                        slot.badge.set_text(te, &row_data.badge);
                        slot.badge_width = slot.badge.measure(te, None).0;
                    }

                    let row_y = viewport.y + idx as f32 * stride - scroll_val;
                    let row_rect = Rect::new(viewport.x, row_y, viewport.width - 10.0, item_h);
                    let is_sel = selected_idx == Some(idx);
                    let is_hov = hovered_idx == Some(idx);

                    let fill = if is_sel {
                        theme.accent_soft
                    } else if is_hov {
                        theme.control_hover
                    } else {
                        theme.panel_bg
                    };

                    pctx.ctx.push_sdf(SdfQuad {
                        rect: snap_rect_to_pixels(row_rect, scale),
                        radii: CornerRadii::all(8.0).scale(scale),
                        fill,
                        border_color: if is_sel { theme.accent } else { theme.hairline },
                        border_width: 1.0 * scale,
                        ..Default::default()
                    });

                    let slot = &st.pool[slot_idx];
                    pctx.ctx.push_text_colored(
                        &slot.title,
                        row_rect.x + 14.0,
                        row_rect.y + 7.0,
                        if is_sel {
                            theme.accent
                        } else {
                            theme.text_primary
                        },
                    );
                    pctx.ctx.push_text_colored(
                        &slot.subtitle,
                        row_rect.x + 14.0,
                        row_rect.y + 25.0,
                        theme.text_secondary,
                    );
                    pctx.ctx.push_text_colored(
                        &slot.badge,
                        row_rect.x + row_rect.width - slot.badge_width - 14.0,
                        row_rect.y + (item_h - 16.0) * 0.5,
                        theme.text_secondary,
                    );
                }
            }

            pctx.ctx.pop_clip();

            if total_h > viewport.height {
                let track_h = (viewport.height - 8.0).max(0.0);
                let thumb_h =
                    ((viewport.height / total_h) * track_h).clamp(24.0_f32.min(track_h), track_h);
                let max_scroll = (total_h - viewport.height).max(1.0);
                let thumb_t = (scroll_val / max_scroll).clamp(0.0, 1.0);
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
                let track_h = (viewport.height - 8.0).max(0.0);
                let thumb_h = ((viewport.height / *content_height) * track_h)
                    .clamp(24.0_f32.min(track_h), track_h);
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
                    border_width: 1.0 * scale,
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

fn find_tooltip_node(
    wid: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    x: f32,
    y: f32,
    offset: (f32, f32),
) -> Option<(WidgetId, Rect)> {
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
        return find_tooltip_node(*content_id, nodes, x, y, child_offset);
    }

    for &child in node.children.iter().rev() {
        if let Some(res) = find_tooltip_node(child, nodes, x, y, offset) {
            return Some(res);
        }
    }

    if bounds.contains(x, y) && node.tooltip.is_some() {
        return Some((wid, bounds));
    }
    None
}

fn find_context_menu_node(
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
        return find_context_menu_node(*content_id, nodes, x, y, child_offset);
    }

    for &child in node.children.iter().rev() {
        if let Some(hit) = find_context_menu_node(child, nodes, x, y, offset) {
            return Some(hit);
        }
    }

    if bounds.contains(x, y) && !node.context_menu.is_empty() {
        return Some(wid);
    }
    None
}

fn widget_screen_bounds(
    wid: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    target: WidgetId,
    offset: (f32, f32),
) -> Option<Rect> {
    let node = nodes.get(wid)?;
    let bounds = Rect::new(
        node.bounds.x + offset.0,
        node.bounds.y + offset.1,
        node.bounds.width,
        node.bounds.height,
    );
    if wid == target {
        return Some(bounds);
    }
    let child_offset = match &node.payload {
        WidgetPayload::ScrollView { scroll_y, .. } => (offset.0, offset.1 - scroll_y.value),
        _ => offset,
    };
    for &child in &node.children {
        if let Some(found) = widget_screen_bounds(child, nodes, target, child_offset) {
            return Some(found);
        }
    }
    None
}

fn dispatch_scroll_node(
    wid: WidgetId,
    nodes: &mut SlotMap<WidgetId, WidgetNode>,
    cx: f32,
    cy: f32,
    delta_y: f32,
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
    if !bounds.contains(cx, cy) {
        return false;
    }

    let child_offset = match &node.payload {
        WidgetPayload::ScrollView { scroll_y, .. } => (offset.0, offset.1 - scroll_y.value),
        _ => offset,
    };

    let children = node.children.clone();
    for child in children.into_iter().rev() {
        if dispatch_scroll_node(child, nodes, cx, cy, delta_y, child_offset) {
            return true;
        }
    }

    let Some(node) = nodes.get_mut(wid) else {
        return false;
    };
    match &mut node.payload {
        WidgetPayload::VirtualizedList { state } => {
            let mut st = state.borrow_mut();
            let stride = st.item_height + st.gap;
            let total_h = if st.item_count == 0 {
                0.0
            } else {
                st.item_count as f32 * stride - st.gap
            };
            let max_scroll = (total_h - bounds.height).max(0.0);
            if max_scroll > 0.0 {
                let next = (st.scroll_y.target - delta_y).clamp(0.0, max_scroll);
                st.scroll_y.set_target(next);
                return true;
            }
            false
        }
        WidgetPayload::ScrollView {
            scroll_y,
            content_height,
            ..
        } => {
            let max_scroll = (*content_height - bounds.height).max(0.0);
            if max_scroll > 0.0 {
                let next = (scroll_y.target - delta_y).clamp(0.0, max_scroll);
                scroll_y.set_target(next);
                return true;
            }
            false
        }
        WidgetPayload::TextArea {
            text_block,
            scroll_y,
            ..
        } => {
            let line_count = text_block.line_start_offsets().len().max(1);
            let content_h = line_count as f32 * text_block.style().line_height;
            let max_scroll = (content_h - (bounds.height - 20.0)).max(0.0);
            if max_scroll > 0.0 {
                *scroll_y = (*scroll_y - delta_y).clamp(0.0, max_scroll);
                return true;
            }
            false
        }
        _ => false,
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

    if let WidgetPayload::SplitPane {
        direction,
        divider_id,
        ..
    } = &node.payload
        && let Some(div) = nodes.get(*divider_id)
    {
        let div_bounds = Rect::new(
            div.bounds.x + offset.0,
            div.bounds.y + offset.1,
            div.bounds.width,
            div.bounds.height,
        );
        let hit_zone = match direction {
            SplitDirection::Horizontal => {
                Rect::new(div_bounds.x - 3.5, div_bounds.y, 8.0, div_bounds.height)
            }
            SplitDirection::Vertical => {
                Rect::new(div_bounds.x, div_bounds.y - 3.5, div_bounds.width, 8.0)
            }
        };
        if hit_zone.contains(x, y) {
            return Some(wid);
        }
    }

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
                | WidgetPayload::TextArea { .. }
                | WidgetPayload::Tabs { .. }
                | WidgetPayload::Dropdown { .. }
                | WidgetPayload::TrafficLights { .. }
                | WidgetPayload::VirtualizedList { .. }
                | WidgetPayload::Image {
                    on_click: Some(_),
                    ..
                }
                | WidgetPayload::Container {
                    on_click: Some(_),
                    ..
                }
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
                border_width: border_width * scale,
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
                border_width: border_width * scale,
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
