#![cfg(target_os = "windows")]
#![allow(unused_variables)]

use com::{com_interface, interfaces::iunknown::IUnknown};
use libc::{c_char, c_int, c_void};
use std::ffi::{CStr, OsStr};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::ffi::{OsString, CString};
use std::ptr;
use winapi::shared::{basetsd, minwindef, ntdef, windef, winerror, wtypesbase};
use winapi::um::{errhandlingapi, libloaderapi, ole2, wingdi, winuser, combaseapi, shobjidl_core, shobjidl};
use winapi::Interface;
use winreg::{enums, RegKey};
use bitflags::bitflags;
use std::mem;

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
    saved_style: ntdef::LONG,
    saved_ex_style: ntdef::LONG,
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
        let result = ole2::OleInitialize(ptr::null_mut());

        if result != winerror::S_OK && result != winerror::S_FALSE {
            return ptr::null_mut();
        }

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

unsafe fn from_wstring(wide: *const u16) -> OsString {
    assert!(!wide.is_null());
    for i in 0.. {
        if *wide.offset(i) == 0 {
            return OsStringExt::from_wide(std::slice::from_raw_parts(wide, i as usize));
        }
    }
    unreachable!()
}

#[no_mangle]
extern "C" fn webview_set_color(webview: *mut WebView, r: u8, g: u8, b: u8, a: u8) {
    unsafe {
        let brush = wingdi::CreateSolidBrush(wingdi::RGB(r, g, b));
        winuser::SetClassLongPtrW(
            (*webview).hwnd,
            winuser::GCLP_HBRBACKGROUND,
            std::mem::transmute(brush),
        );
    }
}

#[no_mangle]
extern "C" fn webview_set_title(webview: *mut WebView, title: *const c_char) {
    unsafe {
        let c_title = CStr::from_ptr(title);
        let title = c_title.to_string_lossy();
        let title = to_wstring(&title);
        winuser::SetWindowTextW((*webview).hwnd, title.as_ptr());
    }
}

#[no_mangle]
extern "C" fn webview_set_fullscreen(webview: *mut WebView, fullscreen: c_int) {
    unsafe {
        let mut webview = webview.as_mut().unwrap();

        if webview.is_fullscreen == 0 {
            webview.saved_style = winuser::GetWindowLongW(webview.hwnd, winuser::GWL_STYLE);
            webview.saved_ex_style = winuser::GetWindowLongW(webview.hwnd, winuser::GWL_EXSTYLE);
            winuser::GetWindowRect(webview.hwnd, &mut webview.saved_rect as *mut _);
        }

        webview.is_fullscreen = fullscreen;
        if fullscreen > 0 {
            let mut monitor_info: winuser::MONITORINFO = Default::default();
            monitor_info.cbSize = std::mem::size_of::<winuser::MONITORINFO>() as minwindef::DWORD;

            winuser::SetWindowLongW(
                webview.hwnd,
                winuser::GWL_STYLE,
                webview.saved_style & !(winuser::WS_CAPTION | winuser::WS_THICKFRAME) as i32,
            );

            winuser::SetWindowLongW(
                webview.hwnd,
                winuser::GWL_EXSTYLE,
                webview.saved_ex_style
                    & !(winuser::WS_EX_DLGMODALFRAME
                        | winuser::WS_EX_WINDOWEDGE
                        | winuser::WS_EX_CLIENTEDGE
                        | winuser::WS_EX_STATICEDGE) as i32,
            );

            let monitor = winuser::MonitorFromWindow(webview.hwnd, winuser::MONITOR_DEFAULTTONEAREST);
            winuser::GetMonitorInfoW(monitor, &mut monitor_info as *mut _);

            let monitor_rect = monitor_info.rcMonitor;

            winuser::SetWindowPos(
                webview.hwnd,
                ptr::null_mut(),
                monitor_rect.left,
                monitor_rect.top,
                monitor_rect.right - monitor_rect.left,
                monitor_rect.bottom - monitor_rect.top,
                winuser::SWP_NOZORDER | winuser::SWP_NOACTIVATE | winuser::SWP_FRAMECHANGED,
            );
        } else {
            winuser::SetWindowLongW(webview.hwnd, winuser::GWL_STYLE, webview.saved_style);
            winuser::SetWindowLongW(webview.hwnd, winuser::GWL_EXSTYLE, webview.saved_ex_style);
            let rect = &webview.saved_rect;
            winuser::SetWindowPos(
                webview.hwnd,
                ptr::null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                winuser::SWP_NOZORDER | winuser::SWP_NOACTIVATE | winuser::SWP_FRAMECHANGED,
            );
        }
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
    use shobjidl::*;

    unsafe {
        match dialog_type {
            DialogType::Open | DialogType::Save => {
                let mut dialog = ptr::null_mut::<shobjidl::IFileDialog>();
                let (dresult, options) = match dialog_type {
                    DialogType::Open => {
                        let dresult = combaseapi::CoCreateInstance(
                            &shobjidl_core::CLSID_FileOpenDialog,
                            ptr::null_mut(),
                            wtypesbase::CLSCTX_INPROC_SERVER,
                            &shobjidl::IFileOpenDialog::uuidof(),
                            mem::transmute(&mut dialog),
                        );

                        let mut options = FOS_NOCHANGEDIR | FOS_ALLNONSTORAGEITEMS | FOS_NOVALIDATE |
                            FOS_PATHMUSTEXIST | FOS_FILEMUSTEXIST | FOS_SHAREAWARE |
                            FOS_NOTESTFILECREATE | FOS_NODEREFERENCELINKS |
                            FOS_FORCESHOWHIDDEN | FOS_DEFAULTNOMINIMODE;

                        if flags == DialogFlags::DIRECTORY {
                            options |= FOS_PICKFOLDERS;
                        }

                        (dresult, options)
                    },
                    DialogType::Save => (
                        combaseapi::CoCreateInstance(
                            &shobjidl_core::CLSID_FileSaveDialog,
                            ptr::null_mut(),
                            wtypesbase::CLSCTX_INPROC_SERVER,
                            &shobjidl::IFileSaveDialog::uuidof(),
                            mem::transmute(&mut dialog),
                        ),
                        FOS_OVERWRITEPROMPT | FOS_NOCHANGEDIR |
                        FOS_ALLNONSTORAGEITEMS | FOS_NOVALIDATE | FOS_SHAREAWARE |
                        FOS_NOTESTFILECREATE | FOS_NODEREFERENCELINKS |
                        FOS_FORCESHOWHIDDEN | FOS_DEFAULTNOMINIMODE
                    ),
                    DialogType::Alert => unreachable!(),
                };

                if dresult != winerror::S_OK {
                    eprintln!("could not create dialog");
                    (*dialog).Release();
                    return;
                }

                let mut base_options = 0;
                let dresult = (*dialog).GetOptions(&mut base_options);
                if dresult != winerror::S_OK {
                    eprintln!("could not get dialog options");
                    (*dialog).Release();
                    return;
                }

                base_options &= !FOS_NOREADONLYRETURN;
                base_options |= options;

                let dresult = (*dialog).SetOptions(base_options);
                if dresult != winerror::S_OK {
                    eprintln!("could not set dialog options");
                    (*dialog).Release();
                    return;
                }

                let dresult = (*dialog).Show((*webview).hwnd);
                if dresult != winerror::S_OK {
                    eprintln!("could not show dialog");
                    (*dialog).Release();
                    return;
                }

                let mut res = ptr::null_mut::<shobjidl_core::IShellItem>();
                let dresult = (*dialog).GetResult(&mut res);
                if dresult != winerror::S_OK {
                    eprintln!("could not get dialog result");
                    (*dialog).Release();
                    return;
                }

                let mut res_path = ptr::null_mut::<u16>();
                let dresult = (*res).GetDisplayName(shobjidl_core::SIGDN_FILESYSPATH, &mut res_path);
                if dresult != winerror::S_OK {
                    eprintln!("could not get dialog result display name");
                    (*res).Release();
                    (*dialog).Release();
                    return;
                }

                if res_path.is_null() {
                    eprintln!("result display name is null");
                    (*res).Release();
                    (*dialog).Release();
                    return;
                }

                let path = from_wstring(res_path);
                combaseapi::CoTaskMemFree(mem::transmute(res_path));
                (*res).Release();
                (*dialog).Release();

                let path = path
                    .into_string()
                    .expect("could not convert string to utf8");

                let path = CString::new(path)
                    .expect("could not convert string to cstring");

                let bytes = path.as_bytes_with_nul();
                ptr::copy(bytes.as_ptr(), mem::transmute(result), bytes.len());
                eprintln!("result {:?}", path);
            },
            DialogType::Alert => {
                //to_wstring()
                match flags {
                    DialogFlags::INFO => winuser::MessageBoxW((*webview).hwnd, ptr::null_mut(), ptr::null_mut(), winuser::MB_ICONINFORMATION),
                    DialogFlags::WARNING => winuser::MessageBoxW((*webview).hwnd, ptr::null_mut(), ptr::null_mut(), winuser::MB_ICONWARNING),
                    DialogFlags::ERROR => winuser::MessageBoxW((*webview).hwnd, ptr::null_mut(), ptr::null_mut(), winuser::MB_ICONERROR),
                    _ => winuser::MessageBoxW((*webview).hwnd, ptr::null_mut(), ptr::null_mut(), winuser::MB_ICONERROR),
                };
            },
        }
    }
}