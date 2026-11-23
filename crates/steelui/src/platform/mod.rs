use std::path::PathBuf;

use crate::core::SteelError;

pub struct Clipboard;

impl Clipboard {
    pub fn get_text() -> Result<String, SteelError> {
        let mut cb = arboard::Clipboard::new()
            .map_err(|e| SteelError::Platform(format!("Clipboard init: {e}")))?;
        cb.get_text()
            .map_err(|e| SteelError::Platform(format!("Clipboard read: {e}")))
    }

    pub fn set_text(text: &str) -> Result<(), SteelError> {
        let mut cb = arboard::Clipboard::new()
            .map_err(|e| SteelError::Platform(format!("Clipboard init: {e}")))?;
        cb.set_text(text)
            .map_err(|e| SteelError::Platform(format!("Clipboard write: {e}")))
    }
}

pub struct FileDialog;

impl FileDialog {
    pub fn pick_file(title: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(title).pick_file()
    }

    pub fn pick_folder(title: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(title).pick_folder()
    }

    pub fn save_file(title: &str, default_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(default_name)
            .save_file()
    }
}
