#![cfg(all(target_family = "unix", not(target_os = "macos")))]

use gdk_sys::{gdk_threads_add_idle, GdkGeometry, GdkRGBA, GDK_HINT_MIN_SIZE};
use gio_sys::GAsyncResult;
use glib_sys::*;
use gobject_sys::{g_signal_connect_data, GObject};
use gtk_sys::*;
use javascriptcore_sys::*;
use libc::{c_char, c_double, c_int, c_void};
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
    frameless: c_int,
    visible: c_int,
    min_width: c_int,
    min_height: c_int,
    hide_instead_of_close: c_int,
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
unsafe extern "C" fn webview_set_title(webview: *mut WebView, title: *const c_char) {
    gtk_window_set_title(mem::transmute((*webview).window), title);
}

#[no_mangle]
unsafe extern "C" fn webview_set_fullscreen(webview: *mut WebView, fullscreen: c_int) {
    if fullscreen > 0 {
        gtk_window_fullscreen(mem::transmute((*webview).window));
    } else {
        gtk_window_unfullscreen(mem::transmute((*webview).window));
    }
}

#[no_mangle]
unsafe extern "C" fn webview_set_maximized(webview: *mut WebView, maximize: c_int) {
    if maximize == gtk_window_is_maximized(mem::transmute((*webview).window)) {
        return;
    }
    if maximize > 0 {
        gtk_window_maximize(mem::transmute((*webview).window));
    } else {
        gtk_window_unmaximize(mem::transmute((*webview).window));
    }
}

#[no_mangle]
unsafe extern "C" fn webview_set_minimized(webview: *mut WebView, minimize: c_int) {
    if minimize == gtk_window_is_active(mem::transmute((*webview).window)) {
        return;
    }
    if minimize > 0 {
        gtk_window_iconify(mem::transmute((*webview).window));
    } else {
        gtk_window_deiconify(mem::transmute((*webview).window));
    }
}

#[no_mangle]
unsafe extern "C" fn webview_set_visible(webview: *mut WebView, visible: c_int) {
    if visible != 0 {
        gtk_widget_show_all(mem::transmute((*webview).window));
    } else {
        gtk_widget_hide(mem::transmute((*webview).window));
    }
}

#[no_mangle]
unsafe extern "C" fn webview_new(
    title: *const c_char,
    url: *const c_char,
    width: c_int,
    height: c_int,
    resizable: c_int,
    debug: c_int,
    frameless: c_int,
    visible: c_int,
    min_width: c_int,
    min_height: c_int,
    hide_instead_of_close: c_int,
    external_invoke_cb: ExternalInvokeCallback,
    userdata: *mut c_void,
) -> *mut WebView {
    let w = Box::new(WebView {
        url,
        title,
        width,
        height,
        resizable,
        debug,
        frameless,
        visible,
        min_width,
        min_height,
        hide_instead_of_close,
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

        let window_properties = GdkGeometry {
            min_width,
            min_height,

            max_width: 0,
            max_height: 0,
            base_width: 0,
            base_height: 0,
            width_inc: 0,
            height_inc: 0,
            min_aspect: 0.0,
            max_aspect: 0.0,
            win_gravity: 0,
        };

        gtk_window_set_geometry_hints(
            mem::transmute(window),
            ptr::null_mut(),
            mem::transmute(&window_properties),
            GDK_HINT_MIN_SIZE,
        );
    } else {
        gtk_widget_set_size_request(mem::transmute(window), width, height);
    }

    if frameless > 0 {
        gtk_window_set_decorated(mem::transmute(window), 0);
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
    webkit_web_view_load_uri(
        mem::transmute(webview),
        if url.is_null() {
            b"\0".as_ptr() as *const _
        } else {
            url
        },
    );
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

    if visible != 0 {
        gtk_widget_show_all(window);
    }

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

    if hide_instead_of_close != 0 {
        gtk_widget_hide_on_delete(window);
    }

    w
}

extern "C" fn webview_context_menu_cb(
    _webview: *mut WebKitWebView,
    _default_menu: *mut GtkWidget,
    _hit_test_result: *mut WebKitHitTestResult,
    _triggered_with_keyboard: gboolean,
    _userdata: gboolean,
) -> gboolean {
    GTRUE
}

unsafe extern "C" fn external_message_received_cb(
    _m: *mut WebKitUserContentManager,
    r: *mut WebKitJavascriptResult,
    arg: gpointer,
) {
    let webview: *mut WebView = mem::transmute(arg);
    let context = webkit_javascript_result_get_global_context(r);
    let value = webkit_javascript_result_get_value(r);
    let js = JSValueToStringCopy(context, value, ptr::null_mut());
    let n = JSStringGetMaximumUTF8CStringSize(js);
    let mut s = Vec::new();
    s.reserve(n);
    JSStringGetUTF8CString(js, s.as_mut_ptr(), n);
    ((*webview).external_invoke_cb)(webview, s.as_ptr());
}

#[no_mangle]
unsafe extern "C" fn webview_get_user_data(webview: *mut WebView) -> *mut c_void {
    (*webview).userdata
}

#[no_mangle]
unsafe extern "C" fn webview_free(webview: *mut WebView) {
    let _ = Box::from_raw(webview);
}

#[no_mangle]
unsafe extern "C" fn webview_loop(webview: *mut WebView, blocking: c_int) -> c_int {
    gtk_main_iteration_do(blocking);
    if (*webview).should_exit != 0 {
        gtk_window_close((*webview).window as *mut GtkWindow);
        gtk_main_iteration_do(0);
    }
    (*webview).should_exit
}

#[no_mangle]
unsafe extern "C" fn webview_set_color(webview: *mut WebView, r: u8, g: u8, b: u8, a: u8) {
    let color = GdkRGBA {
        red: r as c_double / 255.0,
        green: g as c_double / 255.0,
        blue: b as c_double / 255.0,
        alpha: a as c_double / 255.0,
    };
    webkit_web_view_set_background_color(mem::transmute((*webview).webview), &color);
}

#[no_mangle]
unsafe extern "C" fn webview_set_zoom_level(webview: *mut WebView, percentage: c_double) {
    webkit_web_view_set_zoom_level(mem::transmute((*webview).webview), percentage);
}

#[no_mangle]
unsafe extern "C" fn webview_set_html(webview: *mut WebView, html: *const c_char) {
    webkit_web_view_load_html(
        mem::transmute((*webview).webview),
        html,
        CStr::from_bytes_with_nul_unchecked(b"").as_ptr(),
    );
}

unsafe extern "C" fn webview_load_changed_cb(
    _webview: *mut WebKitWebView,
    event: WebKitLoadEvent,
    arg: gpointer,
) {
    let w: *mut WebView = mem::transmute(arg);
    if event == WEBKIT_LOAD_FINISHED {
        (*w).ready = 1;
    }
}

unsafe extern "C" fn webview_eval_finished(
    _object: *mut GObject,
    _result: *mut GAsyncResult,
    userdata: gpointer,
) {
    let webview: *mut WebView = mem::transmute(userdata);
    (*webview).js_busy = 0;
}

#[no_mangle]
unsafe extern "C" fn webview_eval(webview: *mut WebView, js: *const c_char) -> c_int {
    while (*webview).ready == 0 {
        g_main_context_iteration(ptr::null_mut(), GTRUE);
    }

    (*webview).js_busy = 1;

    webkit_web_view_run_javascript(
        mem::transmute((*webview).webview),
        js,
        ptr::null_mut(),
        Some(webview_eval_finished),
        mem::transmute(webview),
    );

    while (*webview).js_busy == 1 {
        g_main_context_iteration(ptr::null_mut(), GTRUE);
    }

    0
}

type DispatchFn = extern "C" fn(webview: *mut WebView, arg: *mut c_void);

#[repr(C)]
struct DispatchArg {
    func: DispatchFn,
    webview: *mut WebView,
    arg: *mut c_void,
}

unsafe extern "C" fn webview_dispatch_wrapper(userdata: gpointer) -> gboolean {
    let webview: *mut WebView = mem::transmute(userdata);

    loop {
        let arg: *mut DispatchArg = mem::transmute(g_async_queue_try_pop((*webview).queue));
        if arg.is_null() {
            break;
        }

        ((*arg).func)(webview, (*arg).arg);
        let _ = Box::from_raw(arg);
    }

    GFALSE
}

#[no_mangle]
unsafe extern "C" fn webview_dispatch(webview: *mut WebView, func: DispatchFn, arg: *mut c_void) {
    let arg = Box::new(DispatchArg { func, webview, arg });

    let queue = (*webview).queue;

    g_async_queue_lock(queue);
    g_async_queue_push_unlocked(queue, mem::transmute(Box::into_raw(arg)));

    if g_async_queue_length_unlocked(queue) == 1 {
        gdk_threads_add_idle(Some(webview_dispatch_wrapper), mem::transmute(webview));
    }

    g_async_queue_unlock(queue);
}

#[no_mangle]
unsafe extern "C" fn webview_destroy_cb(_widget: *mut GtkWidget, arg: gpointer) {
    let webview: *mut WebView = mem::transmute(arg);
    if webview.is_null() || (*webview).hide_instead_of_close == 0 {
        webview_exit(webview);
    }
}

#[no_mangle]
unsafe extern "C" fn webview_exit(webview: *mut WebView) {
    (*webview).should_exit = 1;
    webview_loop(webview, 0); // pump the event loop to apply
}

#[no_mangle]
unsafe extern "C" fn webview_print_log(s: *const c_char) {
    let format = std::ffi::CString::new("%s\n").unwrap();
    libc::printf(format.as_ptr(), s);
}
