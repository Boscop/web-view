#![cfg(target_os = "windows")]

use winapi::shared::windef;
use winapi::shared::minwindef;
use libc::{c_int, c_char, c_void};
use com::{com_interface, interfaces::iunknown::IUnknown};
use winreg::{enums, RegKey};

type ExternalInvokeCallback = extern "C" fn(webview: *mut WebView, arg: *const c_char);

#[com_interface(00000112-0000-0000-C000-000000000046)]
trait IOleObject: IUnknown {

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
    browser: *mut *mut dyn IOleObject,
    is_fullscreen: minwindef::BOOL,
    saved_style: minwindef::DWORD,
    saved_ex_style: minwindef::DWORD,
    saved_rect: windef::RECT,
    userdata: *mut c_void,
}

const KEY_FEATURE_BROWSER_EMULATION: &str = "Software\\Microsoft\\Internet Explorer\\Main\\FeatureControl\\FEATURE_BROWSER_EMULATION";

#[no_mangle]
extern "C" fn webview_fix_ie_compat_mode() -> c_int {
    let result = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.file_name().map(|s| s.to_os_string()));

    if result.is_none() {
        eprintln!("could not get executable name");
        return -1;
    }

    let exe_name = result.unwrap();

    let hkcu = RegKey::predef(enums::HKEY_CURRENT_USER);
    let result = hkcu.create_subkey(KEY_FEATURE_BROWSER_EMULATION);

    if result.is_err() {
        eprintln!("could not create regkey {:?}", result);
        return -1;
    }

    let (key, _) = result.unwrap();

    let result = key.set_value(&exe_name, &11000u32);
    if result.is_err() {
        eprintln!("could not set regkey value {:?}", result);
        return -1;
    }

    0
}