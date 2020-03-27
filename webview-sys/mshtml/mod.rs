#![cfg(target_os = "windows")]
#![allow(unused_variables)]

mod interface;
mod web_view;
mod window;

use crate::mshtml::window::WM_WEBVIEW_DISPATCH;
use std::ffi::{CStr, CString, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::ptr;

use libc::{c_char, c_int, c_void};

use percent_encoding::percent_decode_str;
use winapi::{shared::windef::RECT, um::winuser::*};

use web_view::WebView;
use window::DispatchData;
use window::Window;

pub(crate) type ExternalInvokeCallback = extern "C" fn(webview: *mut CWebView, arg: *const c_char);
type ErasedDispatchFn = extern "C" fn(webview: *mut CWebView, arg: *mut c_void);

extern "system" {
    fn OleUninitialize();
}

#[repr(C)]
pub(crate) struct CWebView {
    window: Window,
    webview: Box<WebView>,
    external_invoke_cb: ExternalInvokeCallback,
    userdata: *mut c_void,
}

const KEY_FEATURE_BROWSER_EMULATION: &str =
    "Software\\Microsoft\\Internet Explorer\\Main\\FeatureControl\\FEATURE_BROWSER_EMULATION";

fn fix_ie_compat_mode() -> bool {
    use winreg::{enums, RegKey};

    let result = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.file_name().map(|s| s.to_os_string()));

    if result.is_none() {
        eprintln!("could not get executable name");
        return false;
    }

    let exe_name = result.unwrap();

    let hkcu = RegKey::predef(enums::HKEY_CURRENT_USER);
    let result = hkcu.create_subkey(KEY_FEATURE_BROWSER_EMULATION);

    if result.is_err() {
        eprintln!("could not create regkey {:?}", result);
        return false;
    }

    let (key, _) = result.unwrap();

    let result = key.set_value(&exe_name, &11000u32);
    if result.is_err() {
        eprintln!("could not set regkey value {:?}", result);
        return false;
    }

    true
}

const DATA_URL_PREFIX: &str = "data:text/html,";

#[no_mangle]
unsafe extern "C" fn webview_new(
    title: *const c_char,
    url: *const c_char,
    width: c_int,
    height: c_int,
    resizable: c_int,
    debug: c_int,
    frameless: c_int,
    external_invoke_cb: ExternalInvokeCallback,
    userdata: *mut c_void,
) -> *mut CWebView {
    if !fix_ie_compat_mode() {
        return ptr::null_mut();
    }

    let mut cwebview = Box::new(CWebView {
        window: Window::new(width as _, height as _, resizable > 0, frameless > 0),
        webview: WebView::new(),
        external_invoke_cb,
        userdata,
    });

    cwebview.webview.initialize(
        cwebview.window.handle(),
        RECT {
            left: 0,
            right: width,
            top: 0,
            bottom: height,
        },
    );

    let wv_ptr = Box::into_raw(cwebview);

    (*wv_ptr).webview.set_callback(Some(Box::new(move |result| {
        let c_result = CString::new(result).unwrap();
        external_invoke_cb(wv_ptr, c_result.as_ptr());
    })));

    let url = CStr::from_ptr(url);
    let url = url.to_str().expect("url is not valid utf8");

    if url.starts_with(DATA_URL_PREFIX) {
        let content = percent_decode_str(&url[DATA_URL_PREFIX.len()..])
            .decode_utf8()
            .unwrap();

        (*wv_ptr).webview.navigate("about:blank");
        (*wv_ptr).webview.write(&content);
    } else {
        (*wv_ptr).webview.navigate(url);
    }

    ShowWindow((*wv_ptr).window.handle(), SW_SHOWDEFAULT);

    wv_ptr
}

#[no_mangle]
unsafe extern "C" fn webview_loop(_webview: *mut CWebView, blocking: c_int) -> c_int {
    let mut msg: MSG = Default::default();
    if blocking > 0 {
        if GetMessageW(&mut msg, 0 as _, 0 as _, 0 as _) < 0 {
            return 0;
        }
    } else {
        if PeekMessageW(&mut msg, 0 as _, 0 as _, 0 as _, PM_REMOVE) < 0 {
            return 0;
        }
    }

    if msg.message == WM_QUIT {
        return 1;
    }
    TranslateMessage(&msg);
    DispatchMessageW(&msg);

    0
}

#[no_mangle]
unsafe extern "C" fn webview_eval(webview: *mut CWebView, js: *const c_char) -> c_int {
    let js = CStr::from_ptr(js);
    let js = js.to_str().expect("js is not valid utf8");
    (*webview).webview.eval(js);
    return 0;
}

#[no_mangle]
unsafe extern "C" fn webview_exit(webview: *mut CWebView) {
    DestroyWindow((*webview).window.handle());
    OleUninitialize();
}

#[no_mangle]
unsafe extern "C" fn webview_free(webview: *mut CWebView) {
    let _ = Box::from_raw(webview);
}

#[no_mangle]
unsafe extern "C" fn webview_get_user_data(webview: *mut CWebView) -> *mut c_void {
    (*webview).userdata
}

#[no_mangle]
unsafe extern "C" fn webview_dispatch(
    webview: *mut CWebView,
    f: Option<ErasedDispatchFn>,
    arg: *mut c_void,
) {
    let data = Box::new(DispatchData {
        target: webview,
        func: f.unwrap(),
        arg,
    });
    PostMessageW(
        (*webview).window.handle(),
        WM_WEBVIEW_DISPATCH,
        0,
        Box::into_raw(data) as _,
    );
}

#[no_mangle]
unsafe extern "C" fn webview_set_fullscreen(webview: *mut CWebView, fullscreen: c_int) {
    let fullscreen = fullscreen > 0;
    (*webview).window.set_fullscreen(fullscreen);
}

#[no_mangle]
unsafe extern "C" fn webview_set_title(webview: *mut CWebView, title: *mut c_char) {
    let title = CStr::from_ptr(title);
    let title = title.to_str().expect("title is not valid utf8");
    (*webview).window.set_title(title);
}

#[no_mangle]
unsafe extern "C" fn webview_set_color(
    webview: *mut CWebView,
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
) {
    (*webview).window.set_color(red, green, blue, alpha);
}

fn to_wstring(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect()
}
