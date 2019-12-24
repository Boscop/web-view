#![cfg(all(target_family = "unix", not(target_os = "macos")))]
#![allow(unused_imports)]
#![allow(unused_variables)]

use bitflags::bitflags;
use gdk_sys::{gdk_threads_add_idle, GdkRGBA};
use gio_sys::GAsyncResult;
use glib_sys::*;
use gobject_sys::{g_signal_connect_data, GObject};
use gtk_sys::*;
use javascriptcore_sys::{
    JSStringGetMaximumUTF8CStringSize, JSStringGetUTF8CString, JSStringRelease, JSValueToStringCopy,
};
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
    fn webview_check_url(url: *const c_char) -> *const c_char;

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

extern "C" fn external_message_received_cb(
    m: *mut WebKitUserContentManager,
    r: *mut WebKitJavascriptResult,
    arg: gpointer,
) {
    unsafe {
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
}

#[no_mangle]
extern "C" fn webview_get_user_data(webview: *mut WebView) -> *mut c_void {
    unsafe { (*webview).userdata }
}

#[no_mangle]
extern "C" fn webview_free(webview: *mut WebView) {
    unsafe {
        let _ = Box::from_raw(webview);
    }
}

#[no_mangle]
extern "C" fn webview_loop(webview: *mut WebView, blocking: c_int) -> c_int {
    unsafe {
        gtk_main_iteration_do(blocking);
        (*webview).should_exit
    }
}

#[no_mangle]
extern "C" fn webview_set_color(webview: *mut WebView, r: u8, g: u8, b: u8, a: u8) {
    let color = GdkRGBA {
        red: r as c_double / 255.0,
        green: g as c_double / 255.0,
        blue: b as c_double / 255.0,
        alpha: a as c_double / 255.0,
    };
    unsafe {
        webkit_web_view_set_background_color(mem::transmute((*webview).webview), &color);
    }
}

extern "C" fn webview_load_changed_cb(
    webview: *mut WebKitWebView,
    event: WebKitLoadEvent,
    arg: gpointer,
) {
    unsafe {
        let w: *mut WebView = mem::transmute(arg);
        if event == WEBKIT_LOAD_FINISHED {
            (*w).ready = 1;
        }
    }
}

extern "C" fn webview_eval_finished(
    object: *mut GObject,
    result: *mut GAsyncResult,
    userdata: gpointer,
) {
    unsafe {
        let webview: *mut WebView = mem::transmute(userdata);
        (*webview).js_busy = 0;
    }
}

#[no_mangle]
extern "C" fn webview_eval(webview: *mut WebView, js: *const c_char) -> c_int {
    unsafe {
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
}

type DispatchFn = extern "C" fn(webview: *mut WebView, arg: *mut c_void);

#[repr(C)]
struct DispatchArg {
    func: DispatchFn,
    webview: *mut WebView,
    arg: *mut c_void,
}

extern "C" fn webview_dispatch_wrapper(userdata: gpointer) -> gboolean {
    unsafe {
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
}

#[no_mangle]
extern "C" fn webview_dispatch(webview: *mut WebView, func: DispatchFn, arg: *mut c_void) {
    let arg = Box::new(DispatchArg { func, webview, arg });

    unsafe {
        let queue = (*webview).queue;

        g_async_queue_lock(queue);
        g_async_queue_push_unlocked(queue, mem::transmute(Box::into_raw(arg)));

        if g_async_queue_length_unlocked(queue) == 1 {
            gdk_threads_add_idle(Some(webview_dispatch_wrapper), mem::transmute(webview));
        }

        g_async_queue_unlock(queue);
    }
}

#[repr(C)]
pub enum DialogType {
    Open = 0,
    Save = 1,
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

#[no_mangle]
extern "C" fn webview_dialog(
    webview: *mut WebView,
    dialog_type: DialogType,
    flags: DialogFlags,
    title: *const c_char,
    arg: *const c_char,
    result: *mut c_char,
    resultsz: usize,
) {
    unsafe {
        match dialog_type {
            DialogType::Open | DialogType::Save => {
                let (action, button_text) = match dialog_type {
                    DialogType::Open => {
                        if flags == DialogFlags::DIRECTORY {
                            (
                                GTK_FILE_CHOOSER_ACTION_SELECT_FOLDER,
                                CStr::from_bytes_with_nul_unchecked(b"_Open\0"),
                            )
                        } else {
                            (
                                GTK_FILE_CHOOSER_ACTION_OPEN,
                                CStr::from_bytes_with_nul_unchecked(b"_Open\0"),
                            )
                        }
                    }
                    DialogType::Save => (
                        GTK_FILE_CHOOSER_ACTION_SAVE,
                        CStr::from_bytes_with_nul_unchecked(b"_Save\0"),
                    ),
                    _ => unreachable!(),
                };

                let dialog = gtk_file_chooser_dialog_new(
                    title,
                    mem::transmute((*webview).window),
                    action,
                    CStr::from_bytes_with_nul_unchecked(b"_Cancel\0").as_ptr(),
                    GTK_RESPONSE_CANCEL,
                    button_text.as_ptr(),
                    GTK_RESPONSE_ACCEPT,
                    ptr::null_mut::<c_void>(),
                );

                gtk_file_chooser_set_local_only(mem::transmute(dialog), GFALSE);
                gtk_file_chooser_set_select_multiple(mem::transmute(dialog), GFALSE);
                gtk_file_chooser_set_show_hidden(mem::transmute(dialog), GTRUE);
                gtk_file_chooser_set_do_overwrite_confirmation(mem::transmute(dialog), GTRUE);
                gtk_file_chooser_set_create_folders(mem::transmute(dialog), GTRUE);
                let response = gtk_dialog_run(mem::transmute(dialog));
                if response == GTK_RESPONSE_ACCEPT {
                    let filename = gtk_file_chooser_get_filename(mem::transmute(dialog));
                    g_strlcpy(result, filename, resultsz);
                    g_free(mem::transmute(filename));
                }
                gtk_widget_destroy(dialog);
            }
            DialogType::Alert => {
                let message_type = match flags {
                    DialogFlags::INFO => GTK_MESSAGE_INFO,
                    DialogFlags::WARNING => GTK_MESSAGE_WARNING,
                    DialogFlags::ERROR => GTK_MESSAGE_ERROR,
                    _ => GTK_MESSAGE_OTHER,
                };
                let dialog = gtk_message_dialog_new(
                    mem::transmute((*webview).window),
                    GTK_DIALOG_MODAL,
                    message_type,
                    GTK_BUTTONS_OK,
                    CStr::from_bytes_with_nul_unchecked(b"%s\0").as_ptr(),
                    title,
                );
                gtk_message_dialog_format_secondary_text(
                    mem::transmute(dialog),
                    CStr::from_bytes_with_nul_unchecked(b"%s\0").as_ptr(),
                    arg,
                );

                gtk_dialog_run(mem::transmute(dialog));
                gtk_widget_destroy(dialog);
            }
        }
    }
}
