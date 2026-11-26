#[derive(Debug)]
enum SteelEvent {
    Wake,
    AccessKit(accesskit_winit::Event),
}

impl From<accesskit_winit::Event> for SteelEvent {
    fn from(inner: accesskit_winit::Event) -> Self {
        Self::AccessKit(inner)
    }
}

/// Cursor shape requested from [`WindowDelegate::cursor_style`].
///
/// The application maps these values to platform cursor icons whenever the
/// pointer moves over a window. For a borderless window, the native resize
/// cursor for a window edge takes precedence while the pointer is within six
/// logical pixels of that edge.
///
/// # Examples
///
/// ```
/// use cadmium::window::CursorStyle;
///
/// let style = CursorStyle::Pointer;
/// assert_eq!(style, CursorStyle::Pointer);
/// assert_ne!(style, CursorStyle::Grab);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    /// The platform's ordinary arrow cursor.
    Default,
    /// A pointing-hand cursor for links and clickable controls.
    Pointer,
    /// An open-hand cursor indicating content that can be dragged.
    Grab,
    /// A closed-hand cursor indicating an active drag.
    Grabbing,
    /// A horizontal-resize cursor for column edges.
    ColResize,
    /// A vertical-resize cursor for row edges.
    RowResize,
}

/// A window-level action returned by [`WindowDelegate::on_pointer_down`].
///
/// The event loop applies the command after the delegate's callback returns.
/// Commands take precedence over a redraw request and over the built-in window
/// drag behavior for [`WindowDelegate::is_drag_region`].
///
/// # Examples
///
/// ```
/// use cadmium::window::WindowCommand;
///
/// let command: WindowCommand = WindowCommand::None;
/// assert_eq!(command, WindowCommand::None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    /// Do nothing beyond the redraw requested by the input callback.
    None,
    /// Remove this window from the app.
    ///
    /// If this was the last window, the event loop exits.
    Close,
    /// Minimize this window through the platform window manager.
    Minimize,
    /// Switch this window between maximized and restored state.
    ToggleMaximize,
    /// Request a redraw of every window owned by the [`App`].
    RequestRedrawAll,
}

/// The drawing context passed to [`WindowDelegate::render`] for one frame.
///
/// It provides frame geometry, timing, and pointer position, and records
/// primitives into the frame's [`crate::renderer::DrawList`]. `width` and
/// `height` are logical pixels; multiply by [`Self::scale`] to obtain physical
/// pixels for [`SdfQuad`], [`ImageQuad`] and [`GlassQuad`] rectangles. Text,
/// stroke paths, and clip rectangles are supplied in logical pixels and scaled
/// by the renderer. Primitives recorded here are submitted only after the
/// delegate returns.
///
/// # Examples
///
/// ```
/// use cadmium::core::{Color, CornerRadii, Rect};
/// use cadmium::renderer::SdfQuad;
/// use cadmium::window::{FrameContext, WindowDelegate};
///
/// struct Panel;
///
/// impl WindowDelegate for Panel {
///     fn render(&mut self, ctx: &mut FrameContext<'_>) {
///         ctx.push_sdf(SdfQuad {
///             rect: Rect::new(0.0, 0.0, ctx.width * ctx.scale, ctx.height * ctx.scale),
///             radii: CornerRadii::all(12.0 * ctx.scale),
///             fill: Color::rgba(0.1, 0.2, 0.3, 1.0),
///             ..Default::default()
///         });
///     }
/// }
/// ```
pub struct FrameContext<'a> {
    /// Window width in logical pixels.
    pub width: f32,
    /// Window height in logical pixels.
    pub height: f32,
    /// Scale factor from logical pixels to physical surface pixels.
    pub scale: f32,
    /// Seconds elapsed since the previously presented frame.
    ///
    /// The first frame measures from window creation. The event loop clamps
    /// the delegate's animation delta to `0.0001..=0.05` seconds; this value
    /// is the unclamped presentation interval.
    pub frame_dt: f32,
    /// Pointer position in logical pixels, or [`None`] when the pointer is
    /// outside the window or has not been observed yet.
    pub cursor_pos: Option<(f32, f32)>,
    gpu: &'a mut GpuContext,
    draw_list: DrawList,
}

impl<'a> FrameContext<'a> {
    pub(crate) fn gpu_mut(&mut self) -> &mut GpuContext {
        self.gpu
    }

    /// Returns the window's shared text engine for shaping and measuring text.
    ///
    /// Use this to prepare [`crate::text::TextBlock`] values before passing
    /// them to [`Self::push_text`] or [`Self::push_text_colored`]. The engine is
    /// shared with other windows through [`crate::renderer::GpuContext`].
    pub fn text_engine(&mut self) -> &mut TextEngine {
        &mut self.gpu.text_engine
    }

    /// Records a rounded-box SDF quad in physical pixels.
    ///
    /// See [`SdfQuad`] for the gradient, border, and shadow fields. Consecutive
    /// SDF quads are batched into one draw call.
    pub fn push_sdf(&mut self, quad: SdfQuad) {
        self.draw_list.push_sdf(quad);
    }

    /// Records a textured quad with an uploaded GPU texture.
    ///
    /// `img`'s rectangle is in physical pixels and its
    /// [`crate::renderer::TextureHandle`] must still be registered in the
    /// shared GPU context; released handles cause the batch to be skipped.
    pub fn push_image(&mut self, img: ImageQuad) {
        self.draw_list.push_image(img);
    }

    /// Records a translucent glass quad that samples the backdrop drawn so far.
    ///
    /// Each glass quad is drawn in its own pass so it can see earlier
    /// primitives. See [`crate::renderer::GlassQuad`] for the effect fields and
    /// the renderer module docs for coordinate conventions.
    pub fn push_glass(&mut self, glass: GlassQuad) {
        self.draw_list.push_glass(glass);
    }

    /// Tessellates and records a stroked path.
    ///
    /// Path coordinates and `width` are logical pixels. Use
    /// [`crate::renderer::PathBuilder`] to build a path with the `lyon` version
    /// used by the renderer. If tessellation produces no triangles, nothing is
    /// recorded.
    pub fn push_stroke_path(
        &mut self,
        path: &crate::renderer::LyonPath,
        color: crate::core::Color,
        width: f32,
    ) {
        self.draw_list.push_stroke_path(path, color, width);
    }

    /// Records shaped text with an explicit color override.
    ///
    /// `logical_x` and `logical_y` are the logical-space origin of `block`.
    /// `color` replaces the block style's color for monochrome glyphs; color
    /// glyphs keep their own RGB and only take the override's alpha.
    pub fn push_text_colored(
        &mut self,
        block: &crate::text::TextBlock,
        logical_x: f32,
        logical_y: f32,
        color: Color,
    ) {
        self.draw_list
            .push_text_colored(self.gpu, block, logical_x, logical_y, Some(color));
    }

    /// Pushes a clip rectangle in logical pixels onto the clip stack.
    ///
    /// The new clip is intersected with the current clip and affects only
    /// primitives recorded afterwards. Pair every call with [`Self::pop_clip`].
    pub fn push_clip(&mut self, rect: crate::core::Rect) {
        self.draw_list.push_clip(rect);
    }

    /// Pops the most recent clip rectangle and restores the parent clip.
    ///
    /// Extra pops on an empty stack do not panic; they restore a full-target
    /// scissor.
    pub fn pop_clip(&mut self) {
        self.draw_list.pop_clip();
    }

    /// Multiplies the opacity of all primitives pushed afterwards.
    ///
    /// Nested calls multiply; pair every call with [`Self::pop_opacity`]. This
    /// is used for crossfading overlapping widget subtrees such as
    /// [`crate::widgets::Switcher`] transitions.
    pub fn push_opacity(&mut self, alpha: f32) {
        self.draw_list.push_opacity(alpha);
    }

    /// Restores the opacity in effect before [`Self::push_opacity`].
    pub fn pop_opacity(&mut self) {
        self.draw_list.pop_opacity();
    }

    /// Records shaped text using the color stored in the block's style.
    ///
    /// `logical_x` and `logical_y` are the logical-space origin of `block`.
    /// Glyph masks are rasterized into the shared glyph atlas on demand; if
    /// the atlas must grow, another frame is requested automatically.
    pub fn push_text(&mut self, block: &crate::text::TextBlock, logical_x: f32, logical_y: f32) {
        self.draw_list
            .push_text(self.gpu, block, logical_x, logical_y);
    }
}

