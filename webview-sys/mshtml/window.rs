use std::{ffi::OsString, os::windows::ffi::OsStringExt, ptr};

use libc::c_void;
use winapi::{
    shared::{
        minwindef::{LOWORD, LPARAM, LRESULT, UINT, WORD, WPARAM},
        ntdef::LONG,
        windef::{HWND, RECT},
        winerror::{S_FALSE, S_OK},
    },
    um::{
        errhandlingapi::GetLastError, libloaderapi::GetModuleHandleW, ole2::OleInitialize,
        winuser::*,
    },
};

use super::to_wstring;
use crate::mshtml::CWebView;
use crate::mshtml::web_view::WebView;
use crate::mshtml::ExternalInvokeCallback;
use crate::mshtml::ErasedDispatchFn;

pub(crate) const WM_WEBVIEW_DISPATCH: UINT = WM_APP + 1;
pub(crate) const INVOKE_CALLBACK_MSG: UINT = WM_USER + 1;

extern "system" {
    fn OleUninitialize();
    fn ExitProcess(exit_code: UINT);
}

pub(crate) struct DispatchData {
    pub(crate) target: *mut CWebView,
    pub(crate) func: ErasedDispatchFn,
    pub(crate) arg: *mut c_void,
}

pub(crate) struct Window {
    h_wnd: HWND,
    fullscreen: bool,
    saved_style: LONG,
    saved_ex_style: LONG,
    saved_rect: RECT,
}

impl Window {
    pub(crate) fn new() -> Self {
        // TODO: move some of this logic into some sort of event loop or main application
        // the idea is to have application, that can spawn windows and webviews that
        // can be spawned multiple times into these windows

        unsafe {
            let result = OleInitialize(ptr::null_mut());
            if result != S_OK && result != S_FALSE {
                panic!("could not initialize ole");
            }
            let h_instance = GetModuleHandleW(ptr::null_mut());
            if h_instance.is_null() {
                panic!("could not retrieve module handle");
            }
            let class_name = to_wstring("webview");
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_instance,
                hIcon: ptr::null_mut(),
                hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
                hbrBackground: COLOR_WINDOW as _,
                lpszMenuName: ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };
            if RegisterClassW(&class) == 0 {
                // ignore the "Class already exists" error for multiple windows
                if GetLastError() as u32 != 1410 {
                    OleUninitialize();
                    panic!("could not register window class {}", GetLastError() as u32);
                }
            }
            let title = to_wstring("mshtml_webview");
            let h_wnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                HWND_DESKTOP,
                ptr::null_mut(),
                h_instance,
                ptr::null_mut(),
            );

            Window {
                h_wnd,
                fullscreen: false,
                saved_style: 0,
                saved_ex_style: 0,
                saved_rect: Default::default(),
            }
        }
    }

    pub(crate) fn handle(&self) -> HWND {
        self.h_wnd
    }

    fn save_style(&mut self) {
        unsafe {
            self.saved_style = GetWindowLongW(self.h_wnd, GWL_STYLE);
            self.saved_ex_style = GetWindowLongW(self.h_wnd, GWL_EXSTYLE);
            GetWindowRect(self.h_wnd, &mut self.saved_rect);
        }
    }

    fn restore_style(&self) {
        unsafe {
            SetWindowLongW(self.h_wnd, GWL_STYLE, self.saved_style);
            SetWindowLongW(self.h_wnd, GWL_EXSTYLE, self.saved_ex_style);
            let rect = &self.saved_rect;
            SetWindowPos(
                self.h_wnd,
                ptr::null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }

    pub(crate) fn set_fullscreen(&mut self, fullscreen: bool) {
        if self.fullscreen == fullscreen {
            return;
        }

        if !self.fullscreen {
            self.save_style();
        }

        self.fullscreen = fullscreen;

        if !self.fullscreen {
            self.restore_style();
            return;
        }

        unsafe {
            let mut monitor_info: MONITORINFO = Default::default();
            monitor_info.cbSize = std::mem::size_of::<MONITORINFO>() as _;
            GetMonitorInfoW(
                MonitorFromWindow(self.h_wnd, MONITOR_DEFAULTTONEAREST),
                &mut monitor_info,
            );

            SetWindowLongW(
                self.h_wnd,
                GWL_STYLE,
                self.saved_style & !(WS_CAPTION | WS_THICKFRAME) as LONG,
            );

            SetWindowLongW(
                self.h_wnd,
                GWL_EXSTYLE,
                self.saved_ex_style
                    & !(WS_EX_DLGMODALFRAME
                        | WS_EX_WINDOWEDGE
                        | WS_EX_CLIENTEDGE
                        | WS_EX_STATICEDGE) as LONG,
            );

            let rect = &monitor_info.rcMonitor;
            SetWindowPos(
                self.h_wnd,
                ptr::null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }

    pub(crate) fn set_title(&self, title: &str) {
        let title = to_wstring(title);
        unsafe {
            SetWindowTextW(self.h_wnd, title.as_ptr());
        }
    }
}

const BTN_BACK: WORD = 1;
const BTN_NEXT: WORD = 2;
const BTN_REFRESH: WORD = 3;
const BTN_GO: WORD = 4;
const BTN_EVAL: WORD = 5;
const BTN_WRITE_DOC: WORD = 6;

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    static mut EDIT_HWND: HWND = ptr::null_mut();

    match message {
        WM_COMMAND => {
            let wb_ptr: *mut WebView = std::mem::transmute(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
            if wb_ptr.is_null() {
                return 1;
            }
            let cmd = LOWORD(wparam as _);
            match cmd {
                BTN_BACK => (*wb_ptr).prev(),
                BTN_NEXT => (*wb_ptr).next(),
                BTN_REFRESH => (*wb_ptr).refresh(),
                BTN_GO => {
                    let mut buf: [u16; 4096] = [0; 4096];

                    let len = GetWindowTextW(EDIT_HWND, buf.as_mut_ptr(), buf.len() as _);
                    let len = len as usize;

                    if len == 0 {
                        return 1;
                    }

                    let input = OsString::from_wide(&buf[..len + 1]);
                    (*wb_ptr).navigate(&input.to_string_lossy());
                }
                BTN_EVAL => {
                    (*wb_ptr).eval("external.invoke('test');");
                    // (*wb_ptr).eval("alert('hello');");
                }
                BTN_WRITE_DOC => {
                    (*wb_ptr).write("<p>Hello world!</p>");
                }
                _ => {}
            }

            1
        }
        WM_SIZE => {
            let wb_ptr: *mut WebView = std::mem::transmute(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
            if wb_ptr.is_null() {
                println!("wbptr is null");
                return 1;
            }
            let mut rect: RECT = Default::default();
            GetClientRect(hwnd, &mut rect);
            (*wb_ptr).set_rect(rect);

            1
        }
        WM_DESTROY => {
            ExitProcess(0);
            1
        }
        INVOKE_CALLBACK_MSG => {
            let wb_ptr: *mut WebView = std::mem::transmute(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
            if wb_ptr.is_null() {
                return 1;
            }

            let data: *mut String = std::mem::transmute(lparam);
            let data = Box::from_raw(data);

            1
        }
        WM_WEBVIEW_DISPATCH => {
            let data: Box<DispatchData> = Box::from_raw(lparam as _);
            (data.func)(data.target, data.arg);
            1
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
