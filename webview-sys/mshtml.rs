#![cfg(target_os = "windows")]
#![allow(unused_variables)]

use com::{com_interface, interfaces::iunknown::IUnknown};
use libc::{c_char, c_int, c_void};
use std::ffi::{CStr, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use winapi::shared::{basetsd, minwindef, ntdef, windef};
use winapi::um::{errhandlingapi, libloaderapi, ole2, winuser};
use winreg::{enums, RegKey};

type ExternalInvokeCallback = extern "C" fn(webview: *mut WebView, arg: *const c_char);

// #[com_interface(00000112-0000-0000-C000-000000000046)]
// trait IOleObject: IUnknown {

// }

extern "system" {
    // winapi does not have this function defined, we need to declare it ourselves
    fn OleUninitialize();
}

extern "C" {
    fn DisplayHTMLPage(webview: *mut WebView) -> c_int;
}

#[cfg(target_arch = "x86_64")]
unsafe fn set_window_long(window: windef::HWND, data: basetsd::LONG_PTR) -> basetsd::LONG_PTR {
    winuser::SetWindowLongPtrW(window, winuser::GWLP_USERDATA, data)
}

#[cfg(target_arch = "x86")]
unsafe fn set_window_long(window: windef::HWND, data: ntdef::LONG) -> ntdef::LONG {
    winuser::SetWindowLongW(window, winuser::GWLP_USERDATA, data)
}

#[repr(C)]
struct WebView {
    url: *const c_char,
    width: c_int,
    height: c_int,
    resizable: c_int,
    debug: c_int,
    external_invoke_cb: ExternalInvokeCallback,
    hwnd: windef::HWND,
    browser: *mut *mut c_void, // TODO: this needs to be IOleObject
    is_fullscreen: minwindef::BOOL,
    saved_style: minwindef::DWORD,
    saved_ex_style: minwindef::DWORD,
    saved_rect: windef::RECT,
    userdata: *mut c_void,
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
    external_invoke_cb: ExternalInvokeCallback,
    userdata: *mut c_void,
) -> *mut WebView {
    if !fix_ie_compat_mode() {
        return ptr::null_mut();
    }

    unsafe {
        ole2::OleInitialize(ptr::null_mut());

        let class_name = to_wstring("webview");
        let h_instance = libloaderapi::GetModuleHandleA(ptr::null());

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
        let mut style = winuser::WS_OVERLAPPEDWINDOW;

        if resizable == 0 {
            style = winuser::WS_OVERLAPPED
                | winuser::WS_CAPTION
                | winuser::WS_MINIMIZEBOX
                | winuser::WS_SYSMENU;
        }

        let mut rect = windef::RECT {
            left: 0,
            right: width as ntdef::LONG,
            top: 0,
            bottom: height as ntdef::LONG,
        };

        winuser::AdjustWindowRect(&mut rect, winuser::WS_OVERLAPPEDWINDOW, 0);
        let c_title = CStr::from_ptr(title);
        let title = c_title.to_string_lossy();
        let title = to_wstring(&title);

        let webview = Box::new(WebView {
            url,
            width,
            height,
            resizable,
            debug,
            external_invoke_cb,
            hwnd: ptr::null_mut(),
            browser: ptr::null_mut(),
            is_fullscreen: 0,
            saved_style: 0,
            saved_ex_style: 0,
            saved_rect: rect,
            userdata,
        });

        let webview_ptr = Box::into_raw(webview);

        let handle = winuser::CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            style,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            rect.right,
            rect.bottom,
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

        set_window_long(handle, std::mem::transmute(webview_ptr));

        DisplayHTMLPage(webview_ptr);

        winuser::ShowWindow(handle, winuser::SW_SHOWDEFAULT);

        webview_ptr
    }
}

extern "system" {
    fn wndproc(
        window: windef::HWND,
        msg: minwindef::UINT,
        wparam: minwindef::WPARAM,
        lparam: minwindef::LPARAM,
    ) -> minwindef::LRESULT;
}

fn to_wstring(s: &str) -> Vec<u16> {
    let v: Vec<u16> = OsStr::new(s)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();
    v
}
