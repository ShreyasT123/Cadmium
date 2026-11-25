use std::cell::RefCell;
use std::path::PathBuf;

use crate::core::SteelError;

thread_local! {
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

pub struct Clipboard;

impl Clipboard {
    fn with_instance<R>(
        f: impl Fn(&mut arboard::Clipboard) -> Result<R, arboard::Error>,
    ) -> Result<R, SteelError> {
        CLIPBOARD.with(|slot| {
            let mut guard = slot.borrow_mut();
            if guard.is_none() {
                *guard = Some(
                    arboard::Clipboard::new()
                        .map_err(|e| SteelError::Platform(format!("Clipboard init: {e}")))?,
                );
            }
            match f(guard.as_mut().expect("clipboard initialized")) {
                Ok(v) => Ok(v),
                Err(_) => {
                    // Reconnect once if display server/clipboard owner reset
                    *guard = Some(
                        arboard::Clipboard::new()
                            .map_err(|e| SteelError::Platform(format!("Clipboard init: {e}")))?,
                    );
                    f(guard.as_mut().expect("clipboard reinitialized"))
                        .map_err(|e| SteelError::Platform(format!("Clipboard op: {e}")))
                }
            }
        })
    }

    pub fn get_text() -> Result<String, SteelError> {
        Self::with_instance(|cb| cb.get_text())
    }

    pub fn set_text(text: &str) -> Result<(), SteelError> {
        Self::with_instance(|cb| cb.set_text(text))
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
