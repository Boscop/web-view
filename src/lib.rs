//! [![Build Status]][travis] [![Latest Version]][crates.io]
//!
//! [Build Status]: https://api.travis-ci.org/Boscop/web-view.svg?branch=master
//! [travis]: https://travis-ci.org/Boscop/web-view
//! [Latest Version]: https://img.shields.io/crates/v/web-view.svg
//! [crates.io]: https://crates.io/crates/web-view
//!
//! This library provides Rust bindings for the [webview](https://github.com/zserge/webview) library
//! to allow easy creation of cross-platform Rust desktop apps with GUIs based on web technologies.
//!
//! It supports two-way bindings for communication between the Rust backend and JavaScript frontend.
//!
//! It uses Cocoa/WebKit on macOS, gtk-webkit2 on Linux and MSHTML (IE10/11) on Windows, so your app
//! will be **much** leaner than with Electron.
//!
//! To use a custom version of webview, define an environment variable WEBVIEW_DIR with the path to
//! its source directory.
//!
//! For usage info please check out [the examples] and the [original readme].
//!
//! [the examples]: https://github.com/Boscop/web-view/tree/master/examples
//! [original readme]: https://github.com/zserge/webview/blob/master/README.md

#![allow(deprecated)] // TODO: remove this when removing dialogs

extern crate boxfnonce;
extern crate tinyfiledialogs as tfd;
extern crate urlencoding;
extern crate webview_sys as ffi;

mod color;
mod dialog;
mod error;
mod escape;

pub use color::Color;
pub use dialog::DialogBuilder;
pub use error::{CustomError, Error, WVResult};
pub use escape::escape;

use boxfnonce::SendBoxFnOnce;
use ffi::*;
use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    mem,
    os::raw::*,
    sync::{Arc, RwLock, Weak},
};
use urlencoding::encode;

/// JavaScript function used to insert new css rules to webview.
/// This function should be called with only one argument
/// and that is the css to insert int webview.
/// With every call of this function new style element
/// will get created with css pasted as its children.
const CSS_INJECT_FUNCTION: &str = "(function(e){var \
    t=document.createElement('style'),d=document.head||document.\
    getElementsByTagName('head')[0];t.setAttribute('type','text/\
    css'),t.styleSheet?t.styleSheet.cssText=e:t.appendChild(document.\
    createTextNode(e)),d.appendChild(t)})";

/// Content displayable inside a [`WebView`].
///
/// # Variants
///
/// - `Url` - Content to be fetched from a URL.
/// - `Html` - A string containing literal HTML.
///
/// [`WebView`]: struct.WebView.html
#[derive(Debug)]
pub enum Content<T> {
    Url(T),
    Html(T),
}

/// Builder for constructing a [`WebView`] instance.
///
/// # Example
///
/// ```no_run
/// extern crate web_view;
///
/// use web_view::*;
///
/// fn main() {
///     WebViewBuilder::new()
///         .title("Minimal webview example")
///         .content(Content::Url("https://en.m.wikipedia.org/wiki/Main_Page"))
///         .size(800, 600)
///         .resizable(true)
///         .debug(true)
///         .user_data(())
///         .invoke_handler(|_webview, _arg| Ok(()))
///         .build()
///         .unwrap()
///         .run()
///         .unwrap();
/// }
/// ```
///
/// [`WebView`]: struct.WebView.html
pub struct WebViewBuilder<'a, T: 'a, I, C> {
    pub title: &'a str,
    pub content: Option<Content<C>>,
    pub width: i32,
    pub height: i32,
    pub resizable: bool,
    pub debug: bool,
    pub invoke_handler: Option<I>,
    pub user_data: Option<T>,
    pub frameless: bool,
    pub visible: bool,
    pub min_width: i32,
    pub min_height: i32,
}

impl<'a, T: 'a, I, C> Default for WebViewBuilder<'a, T, I, C>
where
    I: FnMut(&mut WebView<T>, &str) -> WVResult + 'a,
    C: AsRef<str>,
{
    fn default() -> Self {
        #[cfg(debug_assertions)]
        let debug = true;
        #[cfg(not(debug_assertions))]
        let debug = false;

        WebViewBuilder {
            title: "Application",
            content: None,
            width: 800,
            height: 600,
            resizable: true,
            debug,
            invoke_handler: None,
            user_data: None,
            frameless: false,
            visible: true,
            min_width: 300,
            min_height: 300,
        }
    }
}

impl<'a, T: 'a, I, C> WebViewBuilder<'a, T, I, C>
where
    I: FnMut(&mut WebView<T>, &str) -> WVResult + 'a,
    C: AsRef<str>,
{
    /// Alias for [`WebViewBuilder::default()`].
    ///
    /// [`WebViewBuilder::default()`]: struct.WebviewBuilder.html#impl-Default
    pub fn new() -> Self {
        WebViewBuilder::default()
    }

    /// Sets the title of the WebView window.
    ///
    /// Defaults to `"Application"`.
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// Sets the content of the WebView. Either a URL or a HTML string.
    pub fn content(mut self, content: Content<C>) -> Self {
        self.content = Some(content);
        self
    }

    /// Sets the size of the WebView window.
    ///
    /// Defaults to 800 x 600.
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Sets the resizability of the WebView window. If set to false, the window cannot be resized.
    ///
    /// Defaults to `true`.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Enables or disables debug mode.
    ///
    /// Defaults to `true` for debug builds, `false` for release builds.
    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }
    /// The window crated will be frameless
    ///
    /// defaults to `false`
    pub fn frameless(mut self, frameless: bool) -> Self {
        self.frameless = frameless;
        self
    }

    /// Set the visibility of the WebView window.
    ///
    /// defaults to `true`
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Sets the minimum size of the WebView window.
    ///
    /// Defaults to 300 x 300.
    pub fn min_size(mut self, width: i32, height: i32) -> Self {
        self.min_width = width;
        self.min_height = height;
        self
    }

    /// Sets the invoke handler callback. This will be called when a message is received from
    /// JavaScript.
    ///
    /// # Errors
    ///
    /// If the closure returns an `Err`, it will be returned on the next call to [`step()`].
    ///
    /// [`step()`]: struct.WebView.html#method.step
    pub fn invoke_handler(mut self, invoke_handler: I) -> Self {
        self.invoke_handler = Some(invoke_handler);
        self
    }

    /// Sets the initial state of the user data. This is an arbitrary value stored on the WebView
    /// thread, accessible from dispatched closures without synchronization overhead.
    pub fn user_data(mut self, user_data: T) -> Self {
        self.user_data = Some(user_data);
        self
    }

    /// Validates provided arguments and returns a new WebView if successful.
    pub fn build(self) -> WVResult<WebView<'a, T>> {
        macro_rules! require_field {
            ($name:ident) => {
                self.$name
                    .ok_or_else(|| Error::UninitializedField(stringify!($name)))?
            };
        }

        let title = CString::new(self.title)?;
        let content = require_field!(content);
        let url = match content {
            Content::Url(url) => CString::new(url.as_ref())?,
            Content::Html(html) => {
                CString::new(format!("data:text/html,{}", encode(html.as_ref())))?
            }
        };
        let user_data = require_field!(user_data);
        let invoke_handler = require_field!(invoke_handler);

        WebView::new(
            &title,
            &url,
            self.width,
            self.height,
            self.resizable,
            self.debug,
            self.frameless,
            self.visible,
            self.min_width,
            self.min_height,
            user_data,
            invoke_handler,
        )
    }

    /// Validates provided arguments and runs a new WebView to completion, returning the user data.
    ///
    /// Equivalent to `build()?.run()`.
    pub fn run(self) -> WVResult<T> {
        self.build()?.run()
    }
}

/// Constructs a new builder for a [`WebView`].
///
/// Alias for [`WebViewBuilder::default()`].
///
/// [`WebView`]: struct.Webview.html
/// [`WebViewBuilder::default()`]: struct.WebviewBuilder.html#impl-Default
pub fn builder<'a, T, I, C>() -> WebViewBuilder<'a, T, I, C>
where
    I: FnMut(&mut WebView<T>, &str) -> WVResult + 'a,
    C: AsRef<str>,
{
    WebViewBuilder::new()
}

struct UserData<'a, T> {
    inner: T,
    live: Arc<RwLock<()>>,
    invoke_handler: Box<dyn FnMut(&mut WebView<T>, &str) -> WVResult + 'a>,
    result: WVResult,
}

/// An owned webview instance.
///
/// Construct via a [`WebViewBuilder`].
///
/// [`WebViewBuilder`]: struct.WebViewBuilder.html
#[derive(Debug)]
pub struct WebView<'a, T: 'a> {
    inner: Option<*mut CWebView>,
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, T> WebView<'a, T> {
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::too_many_arguments))]
    fn new<I>(
        title: &CStr,
        url: &CStr,
        width: i32,
        height: i32,
        resizable: bool,
        debug: bool,
        frameless: bool,
        visible: bool,
        min_width: i32,
        min_height: i32,
        user_data: T,
        invoke_handler: I,
    ) -> WVResult<WebView<'a, T>>
    where
        I: FnMut(&mut WebView<T>, &str) -> WVResult + 'a,
    {
        let user_data = Box::new(UserData {
            inner: user_data,
            live: Arc::new(RwLock::new(())),
            invoke_handler: Box::new(invoke_handler),
            result: Ok(()),
        });
        let user_data_ptr = Box::into_raw(user_data);

        unsafe {
            let inner = webview_new(
                title.as_ptr(),
                url.as_ptr(),
                width,
                height,
                resizable as _,
                debug as _,
                frameless as _,
                visible as _,
                min_width,
                min_height,
                Some(ffi_invoke_handler::<T>),
                user_data_ptr as _,
            );

            if inner.is_null() {
                Box::<UserData<T>>::from_raw(user_data_ptr);
                Err(Error::Initialization)
            } else {
                Ok(WebView::from_ptr(inner))
            }
        }
    }

    unsafe fn from_ptr(inner: *mut CWebView) -> WebView<'a, T> {
        WebView {
            inner: Some(inner),
            _phantom: PhantomData,
        }
    }

    /// Creates a thread-safe [`Handle`] to the `WebView`, from which closures can be dispatched.
    ///
    /// [`Handle`]: struct.Handle.html
    pub fn handle(&self) -> Handle<T> {
        Handle {
            inner: self.inner.unwrap(),
            live: Arc::downgrade(&self.user_data_wrapper().live),
            _phantom: PhantomData,
        }
    }

    fn user_data_wrapper_ptr(&self) -> *mut UserData<'a, T> {
        unsafe { webview_get_user_data(self.inner.unwrap()) as _ }
    }

    fn user_data_wrapper(&self) -> &UserData<'a, T> {
        unsafe { &(*self.user_data_wrapper_ptr()) }
    }

    fn user_data_wrapper_mut(&mut self) -> &mut UserData<'a, T> {
        unsafe { &mut (*self.user_data_wrapper_ptr()) }
    }

    /// Borrows the user data immutably.
    pub fn user_data(&self) -> &T {
        &self.user_data_wrapper().inner
    }

    /// Borrows the user data mutably.
    pub fn user_data_mut(&mut self) -> &mut T {
        &mut self.user_data_wrapper_mut().inner
    }

    /// Window handle (Windows only)
    pub fn window_handle(&self) -> *mut c_void {
        unsafe { webview_get_window_handle(self.inner.unwrap()) as _ }
    }

    #[deprecated(note = "Please use exit instead")]
    pub fn terminate(&mut self) {
        self.exit();
    }

    /// Gracefully exits the webview
    pub fn exit(&mut self) {
        unsafe { webview_exit(self.inner.unwrap()) }
    }

    /// Executes the provided string as JavaScript code within the `WebView` instance.
    pub fn eval(&mut self, js: &str) -> WVResult {
        let js = CString::new(js)?;
        let ret = unsafe { webview_eval(self.inner.unwrap(), js.as_ptr()) };
        if ret != 0 {
            Err(Error::JsEvaluation)
        } else {
            Ok(())
        }
    }

    /// Injects the provided string as CSS within the `WebView` instance.
    pub fn inject_css(&mut self, css: &str) -> WVResult {
        let inject_func = format!("{}({})", CSS_INJECT_FUNCTION, escape(css));
        self.eval(&inject_func).map_err(|_| Error::CssInjection)
    }

    /// Sets the color of the title bar.
    ///
    /// # Examples
    ///
    /// Without specifying alpha (defaults to 255):
    /// ```ignore
    /// webview.set_color((123, 321, 213));
    /// ```
    ///
    /// Specifying alpha:
    /// ```ignore
    /// webview.set_color((123, 321, 213, 127));
    /// ```
    pub fn set_color<C: Into<Color>>(&mut self, color: C) {
        let color = color.into();
        unsafe { webview_set_color(self.inner.unwrap(), color.r, color.g, color.b, color.a) }
    }

    /// Sets the page native browser zoom level.
    pub fn set_zoom_level(&mut self, percentage: f64) {
        unsafe { webview_set_zoom_level(self.inner.unwrap(), percentage) }
    }

    /// Sets the page HTML directly.
    ///
    /// # Errors
    ///
    /// If `html` contain a nul byte, returns [`Error::NulByte`].
    ///
    /// [`Error::NulByte`]: enum.Error.html#variant.NulByte
    pub fn set_html(&mut self, html: &str) -> WVResult {
        let html = CString::new(html)?;
        unsafe { webview_set_html(self.inner.unwrap(), html.as_ptr()) }
        Ok(())
    }

    /// Sets the title displayed at the top of the window.
    ///
    /// # Errors
    ///
    /// If `title` contain a nul byte, returns [`Error::NulByte`].
    ///
    /// [`Error::NulByte`]: enum.Error.html#variant.NulByte
    pub fn set_title(&mut self, title: &str) -> WVResult {
        let title = CString::new(title)?;
        unsafe { webview_set_title(self.inner.unwrap(), title.as_ptr()) }
        Ok(())
    }

    /// Enables or disables fullscreen.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        unsafe { webview_set_fullscreen(self.inner.unwrap(), fullscreen as _) };
    }

    /// Toggle window maximized
    pub fn set_maximized(&mut self, maximize: bool) {
        unsafe { webview_set_maximized(self.inner.unwrap(), maximize as _) };
    }

    /// Minimizes window
    pub fn set_minimized(&mut self, minimize: bool) {
        unsafe { webview_set_minimized(self.inner.unwrap(), minimize as _) };
    }

    /// Set window visibility.
    pub fn set_visible(&mut self, visible: bool) {
        unsafe { webview_set_visible(self.inner.unwrap(), visible as _) };
    }

    /// Returns a builder for opening a new dialog window.
    #[deprecated(
        note = "Please use crates like 'tinyfiledialogs' for dialog handling, see example in examples/dialog.rs"
    )]
    pub fn dialog<'b>(&'b mut self) -> DialogBuilder<'a, 'b, T> {
        DialogBuilder::new(self)
    }

    /// Iterates the event loop. Returns `None` if the view has been closed or terminated.
    pub fn step(&mut self) -> Option<WVResult> {
        unsafe {
            match webview_loop(self.inner.unwrap(), 1) {
                0 => {
                    let closure_result = &mut self.user_data_wrapper_mut().result;
                    match closure_result {
                        Ok(_) => Some(Ok(())),
                        e => Some(mem::replace(e, Ok(()))),
                    }
                }
                _ => None,
            }
        }
    }

    /// Runs the event loop to completion and returns the user data.
    pub fn run(mut self) -> WVResult<T> {
        loop {
            match self.step() {
                Some(Ok(_)) => (),
                Some(e) => e?,
                None => return Ok(self.into_inner()),
            }
        }
    }

    /// Consumes the `WebView` and returns ownership of the user data.
    pub fn into_inner(mut self) -> T {
        unsafe {
            let user_data = self._into_inner();
            mem::forget(self);
            user_data
        }
    }

    unsafe fn _into_inner(&mut self) -> T {
        let lock = self
            .user_data_wrapper()
            .live
            .write()
            .expect("A dispatch channel thread panicked while holding mutex to WebView.");

        let user_data_ptr = self.user_data_wrapper_ptr();
        webview_exit(self.inner.unwrap());
        webview_free(self.inner.unwrap());
        let user_data = *Box::from_raw(user_data_ptr);
        std::mem::drop(lock);
        user_data.inner
    }
}

impl<'a, T> Drop for WebView<'a, T> {
    fn drop(&mut self) {
        if self.inner.is_some() {
            unsafe {
                self._into_inner();
            }
            self.inner = None;
        }
    }
}

/// A thread-safe handle to a [`WebView`] instance. Used to dispatch closures onto its task queue.
///
/// [`WebView`]: struct.WebView.html
pub struct Handle<T> {
    inner: *mut CWebView,
    live: Weak<RwLock<()>>,
    _phantom: PhantomData<T>,
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Handle {
            inner: self.inner,
            live: self.live.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<T> Handle<T> {
    /// Schedules a closure to be run on the [`WebView`] thread.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Dispatch`] if the [`WebView`] has been dropped.
    ///
    /// If the closure returns an `Err`, it will be returned on the next call to [`step()`].
    ///
    /// [`WebView`]: struct.WebView.html
    /// [`Error::Dispatch`]: enum.Error.html#variant.Dispatch
    /// [`step()`]: struct.WebView.html#method.step
    pub fn dispatch<F>(&self, f: F) -> WVResult
    where
        F: FnOnce(&mut WebView<T>) -> WVResult + Send + 'static,
    {
        // Abort if WebView has been dropped. Otherwise, keep it alive until closure has been
        // dispatched.
        let mutex = self.live.upgrade().ok_or(Error::Dispatch)?;
        let closure = Box::new(SendBoxFnOnce::new(f));
        let _lock = mutex.read().map_err(|_| Error::Dispatch)?;

        // Send closure to webview.
        unsafe {
            webview_dispatch(
                self.inner,
                Some(ffi_dispatch_handler::<T> as _),
                Box::into_raw(closure) as _,
            )
        }
        Ok(())
    }
}

unsafe impl<T> Send for Handle<T> {}
unsafe impl<T> Sync for Handle<T> {}

extern "C" fn ffi_dispatch_handler<T>(webview: *mut CWebView, arg: *mut c_void) {
    unsafe {
        let mut handle = WebView::<T>::from_ptr(webview);
        let result = {
            let callback =
                Box::<SendBoxFnOnce<'static, (&mut WebView<T>,), WVResult>>::from_raw(arg as _);
            callback.call(&mut handle)
        };
        handle.user_data_wrapper_mut().result = result;
        // Do not clean up the webview on drop of the temporary WebView in handle
        handle.inner = None;
    }
}

extern "C" fn ffi_invoke_handler<T>(webview: *mut CWebView, arg: *const c_char) {
    unsafe {
        let arg = CStr::from_ptr(arg).to_string_lossy().to_string();
        let mut handle = WebView::<T>::from_ptr(webview);
        let result = ((*handle.user_data_wrapper_ptr()).invoke_handler)(&mut handle, &arg);
        handle.user_data_wrapper_mut().result = result;
        // Do not clean up the webview on drop of the temporary WebView in handle
        handle.inner = None;
    }
}
