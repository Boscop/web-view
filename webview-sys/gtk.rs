#![cfg(all(target_family = "unix", not(target_os = "macos")))]
#![allow(unused_imports)]
#![allow(unused_variables)]

use libc::{c_char, c_int, c_void};
use std::ptr;
use glib_sys::GAsyncQueue;
use gtk_sys::*;
use std::mem;

type ExternalInvokeCallback = extern "C" fn(webview: *mut WebView, arg: *const c_char);

#[repr(C)]
struct WebView {
    url: *const char,
    title: *const char,
    width: c_int,
    height: c_int,
    resizable: c_int,
    debug: c_int,
    external_invoke_cb: ExternalInvokeCallback,
    window: *mut GtkWidget,
    scroller: *mut GtkWidget,
    webview: *mut GtkWidget,
    inspector_window: *mut GtkWidget,
    queue: *mut GAsyncQueue,
    ready: c_int,
    js_busy: c_int,
    should_exit: c_int,
    userdata: *mut c_void,
}

#[no_mangle]
extern "C" fn webview_set_title(webview: *mut WebView, title: *const c_char) {
    unsafe {
        gtk_window_set_title(mem::transmute((*webview).window), title);
    }
}

#[no_mangle]
extern "C" fn webview_set_fullscreen(webview: *mut WebView, fullscreen: c_int) {
    unsafe {
        if fullscreen > 0 {
            gtk_window_fullscreen(mem::transmute((*webview).window));
        } else {
            gtk_window_unfullscreen(mem::transmute((*webview).window));
        }
    }
}