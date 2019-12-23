#![cfg(all(target_family = "unix", not(target_os = "macos")))]
#![allow(unused_imports)]
#![allow(unused_variables)]

use glib_sys::*;
use gobject_sys::g_signal_connect_data;
use gtk_sys::*;
use libc::{c_char, c_int, c_void};
use std::ffi::CStr;
use std::mem;
use std::ptr;
use webkit2gtk_sys::*;

type ExternalInvokeCallback = extern "C" fn(webview: *mut WebView, arg: *const c_char);

#[repr(C)]
struct WebView {
    url: *const c_char,
    title: *const c_char,
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

#[no_mangle]
extern "C" fn webview_new(
    title: *const c_char,
    url: *const c_char,
    width: c_int,
    height: c_int,
    resizable: c_int,
    debug: c_int,
    external_invoke_cb: ExternalInvokeCallback,
    userdata: *mut c_void,
) -> *mut WebView {
    unsafe {
        let w = Box::new(WebView {
            url,
            title,
            width,
            height,
            resizable,
            debug,
            external_invoke_cb,
            window: ptr::null_mut(),
            scroller: ptr::null_mut(),
            webview: ptr::null_mut(),
            inspector_window: ptr::null_mut(),
            queue: ptr::null_mut(),
            ready: 0,
            js_busy: 0,
            should_exit: 0,
            userdata,
        });

        let w = Box::into_raw(w);

        if gtk_init_check(ptr::null_mut(), ptr::null_mut()) == GFALSE {
            return ptr::null_mut();
        }
        (*w).queue = g_async_queue_new();

        let window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
        gtk_window_set_title(mem::transmute(window), title);
        (*w).window = window;

        if resizable > 0 {
            gtk_window_set_default_size(mem::transmute(window), width, height);
        } else {
            gtk_widget_set_size_request(mem::transmute(window), width, height);
        }

        gtk_window_set_resizable(mem::transmute(window), resizable);
        gtk_window_set_position(mem::transmute(window), GTK_WIN_POS_CENTER);

        let scroller = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
        gtk_container_add(mem::transmute(window), scroller);
        (*w).scroller = scroller;

        let m = webkit_user_content_manager_new();
        webkit_user_content_manager_register_script_message_handler(
            m,
            CStr::from_bytes_with_nul_unchecked(b"external\0").as_ptr(),
        );

        g_signal_connect_data(
            mem::transmute(m),
            CStr::from_bytes_with_nul_unchecked(b"script-message-received::external\0").as_ptr(),
            Some(mem::transmute(external_message_received_cb as *const ())),
            mem::transmute(w),
            None,
            0,
        );

        let webview = webkit_web_view_new_with_user_content_manager(m);
        (*w).webview = webview;
        webkit_web_view_load_uri(mem::transmute(webview), webview_check_url(url));
        g_signal_connect_data(
            mem::transmute(webview),
            CStr::from_bytes_with_nul_unchecked(b"load-changed\0").as_ptr(),
            Some(mem::transmute(webview_load_changed_cb as *const ())),
            mem::transmute(w),
            None,
            0,
        );
        gtk_container_add(mem::transmute(scroller), webview);

        let settings = webkit_web_view_get_settings(mem::transmute(webview));
        // Enable webgl and canvas features.
        webkit_settings_set_enable_webgl(settings, 1);
        webkit_settings_set_enable_accelerated_2d_canvas(settings, 1);

        if debug > 0 {
            webkit_settings_set_enable_write_console_messages_to_stdout(settings, 1);
            webkit_settings_set_enable_developer_extras(settings, 1);
        } else {
            g_signal_connect_data(
                mem::transmute(webview),
                CStr::from_bytes_with_nul_unchecked(b"context-menu\0").as_ptr(),
                Some(mem::transmute(webview_context_menu_cb as *const ())),
                mem::transmute(w),
                None,
                0,
            );
        }

        gtk_widget_show_all(window);

        webkit_web_view_run_javascript(
            mem::transmute(webview),
            CStr::from_bytes_with_nul_unchecked(b"window.external={invoke:function(x){window.webkit.messageHandlers.external.postMessage(x);}}\0").as_ptr(),
            ptr::null_mut(),
            None,
            ptr::null_mut(),
        );

        g_signal_connect_data(
            mem::transmute(window),
            CStr::from_bytes_with_nul_unchecked(b"destroy\0").as_ptr(),
            Some(mem::transmute(webview_destroy_cb as *const ())),
            mem::transmute(w),
            None,
            0,
        );

        w
    }
}

extern "C" {
    fn external_message_received_cb(
        m: *mut WebKitUserContentManager,
        r: *mut WebKitJavascriptResult,
        arg: gpointer,
    );

    fn webview_check_url(url: *const c_char) -> *const c_char;

    fn webview_load_changed_cb(webview: *mut WebKitWebView, event: WebKitLoadEvent, arg: gpointer);

    fn webview_destroy_cb(widget: *mut GtkWidget, arg: gpointer);
}

extern "C" fn webview_context_menu_cb(
    webview: *mut WebKitWebView,
    default_menu: *mut GtkWidget,
    hit_test_result: *mut WebKitHitTestResult,
    triggered_with_keyboard: gboolean,
    userdata: gboolean,
) -> gboolean {
    GTRUE
}
