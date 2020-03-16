use std::path::PathBuf;
use tfd::MessageBoxIcon;
use {WVResult, WebView};

/// A builder for opening a new dialog window.
#[deprecated(
    note = "Please use crates like 'tinyfiledialogs' for dialog handling, see example in examples/dialog.rs"
)]
#[derive(Debug)]
pub struct DialogBuilder<'a: 'b, 'b, T: 'a> {
    webview: &'b mut WebView<'a, T>,
}

impl<'a: 'b, 'b, T: 'a> DialogBuilder<'a, 'b, T> {
    /// Creates a new dialog builder for a WebView.
    pub fn new(webview: &'b mut WebView<'a, T>) -> DialogBuilder<'a, 'b, T> {
        DialogBuilder { webview }
    }

    /// Opens a new open file dialog and returns the chosen file path.
    pub fn open_file<S, P>(&mut self, title: S, default_file: P) -> WVResult<Option<PathBuf>>
    where
        S: Into<String>,
        P: Into<PathBuf>,
    {
        let default_file = default_file.into().into_os_string();
        let default_file = default_file
            .to_str()
            .expect("default_file is not valid utf-8");

        let result = tfd::open_file_dialog(&title.into(), default_file, None).map(|p| p.into());
        Ok(result)
    }

    /// Opens a new save file dialog and returns the chosen file path.
    pub fn save_file(&mut self) -> WVResult<Option<PathBuf>> {
        Ok(tfd::save_file_dialog("", "").map(|p| p.into()))
    }

    /// Opens a new choose directory dialog as returns the chosen directory path.
    pub fn choose_directory<S, P>(
        &mut self,
        title: S,
        default_directory: P,
    ) -> WVResult<Option<PathBuf>>
    where
        S: Into<String>,
        P: Into<PathBuf>,
    {
        let default_directory = default_directory.into().into_os_string();
        let default_directory = default_directory
            .to_str()
            .expect("default_directory is not valid utf-8");

        let result = tfd::select_folder_dialog(&title.into(), default_directory).map(|p| p.into());
        Ok(result)
    }

    /// Opens an info alert dialog.
    pub fn info<TS, MS>(&mut self, title: TS, message: MS) -> WVResult
    where
        TS: Into<String>,
        MS: Into<String>,
    {
        tfd::message_box_ok(&title.into(), &message.into(), MessageBoxIcon::Info);
        Ok(())
    }

    /// Opens a warning alert dialog.
    pub fn warning<TS, MS>(&mut self, title: TS, message: MS) -> WVResult
    where
        TS: Into<String>,
        MS: Into<String>,
    {
        tfd::message_box_ok(&title.into(), &message.into(), MessageBoxIcon::Warning);
        Ok(())
    }

    /// Opens an error alert dialog.
    pub fn error<TS, MS>(&mut self, title: TS, message: MS) -> WVResult
    where
        TS: Into<String>,
        MS: Into<String>,
    {
        tfd::message_box_ok(&title.into(), &message.into(), MessageBoxIcon::Error);
        Ok(())
    }
}
