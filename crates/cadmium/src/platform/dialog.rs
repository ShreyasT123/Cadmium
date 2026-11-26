/// Native, blocking file and folder pickers.
///
/// Each method opens a platform dialog with the supplied title and blocks until
/// the user chooses something or cancels. The returned [`PathBuf`] is the path
/// chosen in the dialog; `None` means it was dismissed without a selection. These
/// functions are convenience wrappers and do not report dialog errors separately.
pub struct FileDialog;

impl FileDialog {
    /// Opens a dialog to pick an existing file.
    ///
    /// Returns `Some(path)` for the selected file or `None` if cancelled.
    /// The dialog's title is `title`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::platform::FileDialog;
    /// use std::path::PathBuf;
    ///
    /// fn open_project() -> Option<PathBuf> {
    ///     FileDialog::pick_file("Open project")
    /// }
    /// ```
    pub fn pick_file(title: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(title).pick_file()
    }

    /// Opens a dialog to pick an existing folder.
    ///
    /// Returns `Some(path)` for the selected directory or `None` if cancelled.
    /// The dialog's title is `title`. The chosen directory can be used to build
    /// paths passed to functions such as [`crate::assets::load_image_file`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::platform::FileDialog;
    /// use std::path::PathBuf;
    ///
    /// fn choose_directory() -> Option<PathBuf> {
    ///     FileDialog::pick_folder("Choose a folder")
    /// }
    /// ```
    pub fn pick_folder(title: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(title).pick_folder()
    }

    /// Opens a save dialog pre-filled with `default_name`.
    ///
    /// `title` is the dialog title. Returns `Some(path)` for the target the user
    /// chose (which may name an existing file to overwrite) or `None` if
    /// cancelled. No file is written by this function.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::platform::FileDialog;
    /// use std::path::PathBuf;
    ///
    /// fn save_project() -> Option<PathBuf> {
    ///     FileDialog::save_file("Save project", "untitled.cadmium")
    /// }
    /// ```
    pub fn save_file(title: &str, default_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(default_name)
            .save_file()
    }
}
