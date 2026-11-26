#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalsUniform {
    viewport_size: [f32; 2],
    scale: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniform {
    texel_size: [f32; 2],
    offset: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawSdfInstance {
    rect: [f32; 4],
    radii: [f32; 4],
    fill_color: [f32; 4],
    fill_color_end: [f32; 4],
    border_color: [f32; 4],
    border_and_grad: [f32; 4],
    shadow_color: [f32; 4],
    shadow_params: [f32; 3],
    _pad1: f32,
}

impl From<SdfQuad> for RawSdfInstance {
    fn from(q: SdfQuad) -> Self {
        let end = q.fill_end.unwrap_or(q.fill);
        Self {
            rect: [q.rect.x, q.rect.y, q.rect.width, q.rect.height],
            radii: q.radii.to_array(),
            fill_color: q.fill.to_array(),
            fill_color_end: end.to_array(),
            border_color: q.border_color.to_array(),
            border_and_grad: [
                q.border_width,
                q.gradient_dir[0],
                q.gradient_dir[1],
                q.rotation,
            ],
            shadow_color: q.shadow_color.to_array(),
            shadow_params: [q.shadow_offset[0], q.shadow_offset[1], q.shadow_blur],
            _pad1: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawImageInstance {
    rect: [f32; 4],
    uv_rect: [f32; 4],
    radii: [f32; 4],
    tint: [f32; 4],
}

impl From<ImageQuad> for RawImageInstance {
    fn from(i: ImageQuad) -> Self {
        Self {
            rect: [i.rect.x, i.rect.y, i.rect.width, i.rect.height],
            uv_rect: i.uv_rect,
            radii: i.radii.to_array(),
            tint: i.tint.to_array(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawGlassInstance {
    rect: [f32; 4],
    radii: [f32; 4],
    tint_color: [f32; 4],
    border_color: [f32; 4],
    border_width: f32,
    saturation: f32,
    specular: f32,
    frost: f32,
    glass_fx: [f32; 4],
}

impl From<GlassQuad> for RawGlassInstance {
    fn from(g: GlassQuad) -> Self {
        Self {
            rect: [g.rect.x, g.rect.y, g.rect.width, g.rect.height],
            radii: g.radii.to_array(),
            tint_color: g.tint.to_array(),
            border_color: g.border_color.to_array(),
            border_width: g.border_width,
            saturation: g.saturation,
            specular: g.specular,
            frost: g.frost,
            glass_fx: [
                g.liquid_ripple,
                g.rim_angle,
                g.sheen_progress,
                g.conic_strength,
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawGlyphInstance {
    rect: [f32; 4],
    uv_rect: [f32; 4],
    color: [f32; 4],
}

/// A vertex of path geometry produced by `lyon` stroke tessellation.
///
/// Positions are in physical pixels after the draw list's scale factor has
/// been applied; the path vertex shader converts them to clip space using
/// the viewport size. Colors are linear RGBA and are premultiplied by the
/// shader when writing to a premultiplied-alpha target.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct PathVertex {
    /// Vertex position as `[x, y]` in physical pixels.
    pub position: [f32; 2],
    /// Vertex color as linear `[r, g, b, a]`.
    pub color: [f32; 4],
}

impl PathVertex {
    /// Creates a vertex at (`x`, `y`) with the given color.
    ///
    /// Coordinates are physical pixels. No premultiplication happens here;
    /// the path shader applies it when writing the fragment color.
    pub fn new(x: f32, y: f32, color: Color) -> Self {
        Self {
            position: [x, y],
            color: color.to_array(),
        }
    }
}

/// A convenience builder for [`LyonPath`] geometry.
///
/// Coordinates passed to this builder are logical; [`DrawList::push_stroke_path`]
/// scales the tessellated vertices by the draw list's scale factor. Calls
/// follow the usual sub-path pattern: [`Self::move_to`] starts a sub-path,
/// drawing commands extend it, and [`Self::close`] or [`Self::finish`] ends
/// it before another [`Self::move_to`].
///
/// # Examples
///
/// ```
/// use cadmium::core::Color;
/// use cadmium::renderer::{DrawList, PathBuilder};
///
/// let mut path = PathBuilder::new();
/// path.move_to(10.0, 10.0);
/// path.line_to(90.0, 10.0);
/// path.quadratic_bezier_to(110.0, 10.0, 110.0, 30.0);
/// path.line_to(110.0, 70.0);
/// path.close();
/// let path = path.build();
///
/// let mut frame = DrawList::new(2.0);
/// frame.push_stroke_path(&path, Color::rgba(1.0, 1.0, 1.0, 1.0), 2.0);
/// ```
pub struct PathBuilder {
    builder: lyon::path::path::Builder,
}

impl PathBuilder {
    /// Creates an empty builder, equivalent to [`LyonPath::builder`].
    pub fn new() -> Self {
        Self {
            builder: LyonPath::builder(),
        }
    }

    /// Starts a new sub-path at (`x`, `y`) in logical coordinates.
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.builder.begin(point(x, y));
    }

    /// Appends a straight line from the current point to (`x`, `y`).
    ///
    /// Call [`Self::move_to`] first to establish a starting point.
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(point(x, y));
    }

    /// Appends a quadratic Bézier segment to (`to_x`, `to_y`) using
    /// (`ctrl_x`, `ctrl_y`) as the control point.
    pub fn quadratic_bezier_to(&mut self, ctrl_x: f32, ctrl_y: f32, to_x: f32, to_y: f32) {
        self.builder
            .quadratic_bezier_to(point(ctrl_x, ctrl_y), point(to_x, to_y));
    }

    /// Appends a cubic Bézier segment to (`to_x`, `to_y`) using
    /// (`c1_x`, `c1_y`) and (`c2_x`, `c2_y`) as control points.
    pub fn cubic_bezier_to(
        &mut self,
        c1_x: f32,
        c1_y: f32,
        c2_x: f32,
        c2_y: f32,
        to_x: f32,
        to_y: f32,
    ) {
        self.builder
            .cubic_bezier_to(point(c1_x, c1_y), point(c2_x, c2_y), point(to_x, to_y));
    }

    /// Ends the current sub-path, connecting its last point back to the
    /// starting point so that it can be filled as a closed shape.
    ///
    /// Start a further sub-path with [`Self::move_to`].
    pub fn close(&mut self) {
        self.builder.end(true);
    }

    /// Ends the current sub-path without closing it, leaving the endpoints
    /// open for stroke-only geometry.
    ///
    /// Start a further sub-path with [`Self::move_to`].
    pub fn finish(&mut self) {
        self.builder.end(false);
    }

    /// Consumes the builder and returns the finished path.
    ///
    /// An open sub-path is ended automatically, as an open sub-path, if
    /// neither [`Self::close`] nor [`Self::finish`] was called.
    pub fn build(self) -> LyonPath {
        self.builder.build()
    }
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self::new()
    }
}

