#![cfg(target_os = "windows")]
#![allow(unused_variables)]

use com::{com_interface, interfaces::iunknown::IUnknown};
use libc::{c_char, c_int, c_void};
use std::ffi::{CStr, OsStr};
use std::ffi::{CString, OsString};
use std::mem;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::ptr;
use winapi::shared::{basetsd, minwindef, ntdef, windef, winerror, wtypesbase};
use winapi::um::{
    combaseapi, errhandlingapi, libloaderapi, ole2, shobjidl, shobjidl_core, winbase, wingdi,
    winuser,
};
use winreg::{enums, RegKey};

type ExternalInvokeCallback = extern "C" fn(webview: *mut WebView, arg: *const c_char);

extern "system" {
    // winapi does not have this function defined, we need to declare it ourselves
    fn OleUninitialize();

    // wndproc is defined in c code
    fn wndproc(
        window: windef::HWND,
        msg: minwindef::UINT,
        wparam: minwindef::WPARAM,
        lparam: minwindef::LPARAM,
    ) -> minwindef::LRESULT;
}

extern "C" {
    fn DisplayHTMLPage(webview: *mut WebView) -> c_int;
}

#[repr(C)]
struct WebView {
    url: *const c_char,
    width: c_int,
    height: c_int,
    resizable: c_int,
    debug: c_int,
    frameless: c_int,
    external_invoke_cb: ExternalInvokeCallback,
    userdata: *mut c_void,
    hwnd: windef::HWND,
    browser: *mut *mut c_void, // TODO: this needs to be IOleObject
    is_fullscreen: minwindef::BOOL,
    saved_style: ntdef::LONG,
    saved_ex_style: ntdef::LONG,
    saved_rect: windef::RECT,
}

const KEY_FEATURE_BROWSER_EMULATION: &str =
    "Software\\Microsoft\\Internet Explorer\\Main\\FeatureControl\\FEATURE_BROWSER_EMULATION";

fn fix_ie_compat_mode() -> bool {
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

#[no_mangle]
extern "C" fn webview_new(
    title: *const c_char,
    url: *const c_char,
    width: c_int,
    height: c_int,
    resizable: c_int,
    debug: c_int,
    frameless: c_int,
    external_invoke_cb: ExternalInvokeCallback,
    userdata: *mut c_void,
) -> *mut WebView {
    if !fix_ie_compat_mode() {
        return ptr::null_mut();
    }

    unsafe {
        let result = ole2::OleInitialize(ptr::null_mut());
        if result != winerror::S_OK && result != winerror::S_FALSE {
            return ptr::null_mut();
        }

        let h_instance = libloaderapi::GetModuleHandleA(ptr::null_mut()); // check A vs W
        if h_instance.is_null() {
            return ptr::null_mut();
        }

        let class_name = to_wstring("webview");
        let class = winuser::WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: ptr::null_mut(),
            hCursor: winuser::LoadCursorW(ptr::null_mut(), winuser::IDC_ARROW),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        if winuser::RegisterClassW(&class) == 0 {
            // ignore the "Class already exists" error for multiple windows
            if errhandlingapi::GetLastError() as u32 != 1410 {
                eprintln!(
                    "Unable to register class, error {}",
                    errhandlingapi::GetLastError() as u32
                );

                OleUninitialize();
                return ptr::null_mut();
            }
        }

        // Return value not checked. If this function fails, simply continue without
        // high DPI support.
        let _ = enable_dpi_awareness();
        let mut style = winuser::WS_OVERLAPPEDWINDOW;
        if resizable == 0 {
            style &= !winuser::WS_SIZEBOX;
        }

        if frameless != 0 {
            style &= !(winuser::WS_SYSMENU
                | winuser::WS_CAPTION
                | winuser::WS_MINIMIZEBOX
                | winuser::WS_MAXIMIZEBOX);
        }

        // Get DPI.
        let screen = winuser::GetDC(ptr::null_mut());
        let dpi = wingdi::GetDeviceCaps(screen, wingdi::LOGPIXELSX);
        winuser::ReleaseDC(ptr::null_mut(), screen);

        let mut rect = windef::RECT {
            left: 0,
            right: winbase::MulDiv(width, dpi, 96),
            top: 0,
            bottom: winbase::MulDiv(height, dpi, 96),
        };

        winuser::AdjustWindowRect(&mut rect, winuser::WS_OVERLAPPEDWINDOW, 0);

        let mut client_rect = windef::RECT::default();
        winuser::GetClientRect(winuser::GetDesktopWindow(), &mut client_rect);
        let left = (client_rect.right / 2) - ((rect.right - rect.left) / 2);
        let top = (client_rect.bottom / 2) - ((rect.bottom - rect.top) / 2);
        rect.right = rect.right - rect.left + left;
        rect.left = left;
        rect.bottom = rect.bottom - rect.top + top;
        rect.top = top;

        let c_title = CStr::from_ptr(title);
        let title = c_title.to_string_lossy();
        let title = to_wstring(&title);

        let webview = Box::new(WebView {
            url,
            width,
            height,
            resizable,
            debug,
            frameless,
            external_invoke_cb,
            userdata,
            hwnd: ptr::null_mut(),
            browser: ptr::null_mut(),
            is_fullscreen: 0,
            saved_style: 0,
            saved_ex_style: 0,
            saved_rect: rect,
        });

        let webview_ptr = Box::into_raw(webview);

        let handle = winuser::CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            style,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            winuser::HWND_DESKTOP,
            ptr::null_mut(),
            h_instance,
            webview_ptr as *mut winapi::ctypes::c_void,
        );

        if handle.is_null() {
            eprintln!(
                "Unable to create window, error {}",
                errhandlingapi::GetLastError() as u32
            );

            let _ = Box::from_raw(webview_ptr); // properly drop webview on failure
            OleUninitialize();
            return ptr::null_mut();
        }

        (*webview_ptr).hwnd = handle;

        winuser::SetWindowLongPtrW(
            handle,
            winuser::GWLP_USERDATA,
            std::mem::transmute(webview_ptr),
        );

        if frameless != 0 {
            winuser::SetWindowLongPtrW(handle, winuser::GWL_STYLE, style as _);
        }

        DisplayHTMLPage(webview_ptr);

        winuser::ShowWindow(handle, winuser::SW_SHOWDEFAULT);
        winuser::UpdateWindow(handle);
        winuser::SetFocus(handle);

        webview_ptr
    }
}

fn enable_dpi_awareness() -> bool {
    type FnSetThreadDpiAwarenessContext = extern "system" fn(
        dpi_context: windef::DPI_AWARENESS_CONTEXT,
    ) -> windef::DPI_AWARENESS_CONTEXT;

    type FnSetProcessDpiAware = extern "system" fn() -> minwindef::BOOL;

    let user32 = "user32.dll";
    let user32 = to_wstring(user32);

    unsafe {
        let hmodule = libloaderapi::GetModuleHandleW(user32.as_ptr());
        if hmodule.is_null() {
            return false;
        }

        let set_thread_dpi_awareness = CString::new("SetThreadDpiAwarenessContext").unwrap();
        let set_thread_dpi_awareness =
            libloaderapi::GetProcAddress(hmodule, set_thread_dpi_awareness.as_ptr());

        if !set_thread_dpi_awareness.is_null() {
            let set_thread_dpi_awareness: FnSetThreadDpiAwarenessContext =
                mem::transmute(set_thread_dpi_awareness);
            if !set_thread_dpi_awareness(windef::DPI_AWARENESS_CONTEXT_SYSTEM_AWARE).is_null() {
                return true;
            }
        }

        let set_process_dpi_aware = CString::new("SetProcessDPIAware").unwrap();
        let set_process_dpi_aware =
            libloaderapi::GetProcAddress(hmodule, set_process_dpi_aware.as_ptr());

        if set_process_dpi_aware.is_null() {
            return false;
        }

        let set_process_dpi_aware: FnSetProcessDpiAware = mem::transmute(set_process_dpi_aware);
        set_process_dpi_aware() != 0
    }
}

fn to_wstring(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect()
}

unsafe fn from_wstring(wide: *const u16) -> OsString {
    assert!(!wide.is_null());
    for i in 0.. {
        if *wide.offset(i) == 0 {
            return OsStringExt::from_wide(std::slice::from_raw_parts(wide, i as usize));
        }
    }
    unreachable!()
}
