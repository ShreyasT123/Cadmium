/// Owns the font database and the glyph-mask cache used to shape and draw text.
///
/// One engine is normally shared by an entire window through
/// [`crate::renderer::GpuContext`]. It is borrowed mutably to create or reshape
/// [`TextBlock`]s and when the renderer rasterizes new glyphs into its atlas.
///
/// # Examples
///
/// ```no_run
/// use cadmium::text::TextEngine;
///
/// let mut engine = TextEngine::new();
/// engine.load_system_fonts();
/// ```
pub struct TextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl TextEngine {
    /// Creates an engine with an `en-US` locale and fonts from the operating system.
    ///
    /// On Windows, Segoe UI's regular, bold, and semibold font files are tried
    /// first; if none are available, all system fonts are loaded instead. On
    /// other platforms the system font database is loaded directly. Missing font
    /// files are ignored, so this constructor does not fail.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::text::TextEngine;
    ///
    /// let engine = TextEngine::new();
    /// # let _ = engine;
    /// ```
    pub fn new() -> Self {
        let mut db = cosmic_text::fontdb::Database::new();

        #[cfg(target_os = "windows")]
        {
            for path in [
                r"C:\Windows\Fonts\segoeui.ttf",
                r"C:\Windows\Fonts\segoeuib.ttf",
                r"C:\Windows\Fonts\segoeuisb.ttf",
            ] {
                let _ = db.load_font_file(path);
            }
            if db.is_empty() {
                db.load_system_fonts();
            } else {
                db.set_sans_serif_family("Segoe UI");
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            db.load_system_fonts();
        }

        Self {
            font_system: FontSystem::new_with_locale_and_db("en-US".to_string(), db),
            swash_cache: SwashCache::new(),
        }
    }

    /// Adds all discoverable system fonts to this engine's font database.
    ///
    /// Existing [`TextBlock`] buffers are not re-shaped automatically; recreate
    /// or update them to pick up newly registered faces.
    pub fn load_system_fonts(&mut self) {
        self.font_system.db_mut().load_system_fonts();
    }

    /// Rasterizes a glyph without consulting the swash cache.
    ///
    /// Used by the renderer when filling its own glyph atlas; `None` means the
    /// glyph could not be rasterized.
    pub(crate) fn rasterize_uncached(&mut self, key: CacheKey) -> Option<SwashImage> {
        self.swash_cache
            .get_image_uncached(&mut self.font_system, key)
    }
}

/// Creates an engine with the default system fonts (see [`TextEngine::new`]).
impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// One shaped glyph and its position, produced for the renderer's glyph atlas.
pub(crate) struct ShapedGlyph {
    /// Key identifying the cached glyph mask.
    pub cache_key: CacheKey,
    /// Horizontal pixel position.
    pub x: i32,
    /// Vertical pixel position.
    pub y: i32,
}

