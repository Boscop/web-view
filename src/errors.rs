use std::ffi::NulError;

/// A WebView error.
#[derive(Debug, Fail)]
pub enum Error {
    /// While attempting to build a WebView instance, a required field was not initialized.
    #[fail(display = "Required field uninitialized: {}.", _0)]
    UninitializedField(&'static str),
    /// An error occurred while initializing a WebView instance.
    #[fail(display = "Webview failed to initialize.")]
    Initialization,
    /// A nul-byte was found in a provided string.
    #[fail(display = "{}", _0)]
    NulByte(#[cause] NulError),
    /// An error occurred while evaluating JavaScript in a WebView instance.
    #[fail(display = "Failed to evaluate JavaScript.")]
    JsEvaluation,
    /// An error occurred while injecting CSS into a WebView instance.
    #[fail(display = "Failed to inject CSS.")]
    CssInjection,
    /// Failure to dispatch a closure to a WebView instance via a handle, likely because the
    /// WebView was dropped.
    #[fail(display = "Closure could not be dispatched. WebView was likely dropped.")]
    Dispatch,
}

/// A WebView result.
pub type WVResult<T = ()> = Result<T, Error>;

impl From<NulError> for Error {
    fn from(e: NulError) -> Error {
        Error::NulByte(e)
    }
}
