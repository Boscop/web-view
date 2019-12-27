//! Raw FFI bindings to [webview].
//!
//! To use a custom version of webview, define an environment variable `WEBVIEW_DIR` with the path
//! to its source directory.
//!
//! [webview]: https://github.com/zserge/webview

#[macro_use]
extern crate bitflags;

use std::os::raw::*;

pub enum CWebView {} // opaque type, only used in ffi pointers

type ErasedExternalInvokeFn = extern "C" fn(webview: *mut CWebView, arg: *const c_char);
type ErasedDispatchFn = extern "C" fn(webview: *mut CWebView, arg: *mut c_void);

#[repr(C)]
pub enum DialogType {
	Open  = 0,
	Save  = 1,
	Alert = 2,
}

bitflags! {
	#[repr(C)]
	pub struct DialogFlags: u32 {
		const FILE      = 0b0000;
		const DIRECTORY = 0b0001;
		const INFO      = 0b0010;
		const WARNING   = 0b0100;
		const ERROR     = 0b0110;
	}
}

extern {
	pub fn webview_free(this: *mut CWebView);
	pub fn webview_new(title: *const c_char, url: *const c_char, width: c_int, height: c_int, resizable: c_int, debug: c_int, external_invoke_cb: Option<ErasedExternalInvokeFn>, userdata: *mut c_void) -> *mut CWebView;
	pub fn webview_loop(this: *mut CWebView, blocking: c_int) -> c_int;
	pub fn webview_exit(this: *mut CWebView);
	pub fn webview_get_user_data(this: *mut CWebView) -> *mut c_void;
	pub fn webview_dispatch(this: *mut CWebView, f: Option<ErasedDispatchFn>, arg: *mut c_void);
	pub fn webview_eval(this: *mut CWebView, js: *const c_char) -> c_int;
	pub fn webview_inject_css(this: *mut CWebView, css: *const c_char) -> c_int;
	pub fn webview_set_title(this: *mut CWebView, title: *const c_char);
	pub fn webview_set_fullscreen(this: *mut CWebView, fullscreen: c_int);
	pub fn webview_set_color(this: *mut CWebView, red: u8, green: u8, blue: u8, alpha: u8);
	pub fn webview_dialog(this: *mut CWebView, dialog_type: DialogType, flags: DialogFlags, title: *const c_char, arg: *const c_char, result: *mut c_char, result_size: usize);
}
