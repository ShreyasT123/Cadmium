/// Access to the system clipboard for the current thread.
///
/// The underlying `arboard` connection is created on first use and kept in
/// thread-local storage. If an operation fails, the connection is recreated once
/// and the operation retried; this recovers from a clipboard owner or display
/// server reset. Only UTF-8 text is exposed, not arbitrary clipboard formats.
///
/// # Examples
///
/// ```no_run
/// use cadmium::platform::Clipboard;
///
/// # fn copy() -> Result<(), cadmium::CadmiumError> {
/// Clipboard::set_text("Hello from Cadmium")?;
/// let text = Clipboard::get_text()?;
/// # let _ = text;
/// # Ok(())
/// # }
/// ```
pub struct Clipboard;

impl Clipboard {
    fn with_instance<R>(
        f: impl Fn(&mut arboard::Clipboard) -> Result<R, arboard::Error>,
    ) -> Result<R, CadmiumError> {
        CLIPBOARD.with(|slot| {
            let mut guard = slot.borrow_mut();
            if guard.is_none() {
                *guard = Some(
                    arboard::Clipboard::new()
                        .map_err(|e| CadmiumError::Platform(format!("Clipboard init: {e}")))?,
                );
            }
            match f(guard.as_mut().expect("clipboard initialized")) {
                Ok(v) => Ok(v),
                Err(_) => {
                    // Reconnect once if display server/clipboard owner reset
                    *guard = Some(
                        arboard::Clipboard::new()
                            .map_err(|e| CadmiumError::Platform(format!("Clipboard init: {e}")))?,
                    );
                    f(guard.as_mut().expect("clipboard reinitialized"))
                        .map_err(|e| CadmiumError::Platform(format!("Clipboard op: {e}")))
                }
            }
        })
    }

    /// Reads UTF-8 text from the system clipboard.
    ///
    /// The clipboard connection is initialized on first use. If the read fails,
    /// the connection is recreated and the read attempted once more.
    ///
    /// # Errors
    ///
    /// Returns [`CadmiumError::Platform`] if the clipboard cannot be opened or the
    /// text cannot be read after the retry. An empty clipboard may be reported as
    /// an error by the platform.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::platform::Clipboard;
    /// use cadmium::CadmiumError;
    ///
    /// # fn paste() -> Result<String, CadmiumError> {
    /// let contents = Clipboard::get_text()?;
    /// # Ok(contents)
    /// # }
    /// ```
    pub fn get_text() -> Result<String, CadmiumError> {
        Self::with_instance(|cb| cb.get_text())
    }

    /// Replaces the system clipboard contents with `text`.
    ///
    /// The connection is initialized on first use, and a failed write is retried
    /// once after reconnecting. Existing clipboard data of other formats may be
    /// discarded by the platform.
    ///
    /// # Errors
    ///
    /// Returns [`CadmiumError::Platform`] if the clipboard cannot be opened or the
    /// text cannot be written after the retry.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::platform::Clipboard;
    /// use cadmium::CadmiumError;
    ///
    /// # fn copy() -> Result<(), CadmiumError> {
    /// Clipboard::set_text("Copied text")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_text(text: &str) -> Result<(), CadmiumError> {
        Self::with_instance(|cb| cb.set_text(text))
    }
}

