//! Native platform services: system clipboard and file dialogs.
//!
//! [`Clipboard`] wraps `arboard` with a lazily initialized, per-thread connection,
//! and [`FileDialog`] wraps `rfd` to present native modal pickers. Both require a
//! desktop session with access to the platform's display server; calls made in a
//! headless context may fail or block. Dialog functions return control only after
//! the user confirms or cancels.
//!
//! # Examples
//!
//! ```no_run
//! use cadmium::platform::{Clipboard, FileDialog};
//!
//! # fn platform_tasks() -> Result<(), cadmium::CadmiumError> {
//! Clipboard::set_text("ready")?;
//! let _text = Clipboard::get_text()?;
//! if let Some(path) = FileDialog::pick_file("Open a file") {
//!     println!("{path:?}");
//! }
//! # Ok(())
//! # }
//! ```

use std::cell::RefCell;
use std::path::PathBuf;

use crate::core::CadmiumError;

thread_local! {
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

include!("clipboard.rs");
include!("dialog.rs");
