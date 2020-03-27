use std::{ffi::CString, ptr};

use libc::c_void;
use winapi::{
    shared::{
        minwindef::{BOOL, LPARAM, LRESULT, UINT, WPARAM},
        ntdef::LONG,
        windef::{DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_SYSTEM_AWARE, HWND, RECT},
        winerror::{S_FALSE, S_OK},
    },
    um::{
        errhandlingapi::GetLastError,
        libloaderapi::{GetModuleHandleW, GetProcAddress},
        ole2::OleInitialize,
        winbase::MulDiv,
        wingdi::{CreateSolidBrush, GetDeviceCaps, LOGPIXELSX, RGB},
        winuser::*,
    },
};

use super::to_wstring;
use crate::mshtml::web_view::WebView;
use crate::mshtml::CWebView;
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
    pub(crate) fn new(width: i32, height: i32, resizable: bool, frameless: bool) -> Self {
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

            let _ = enable_dpi_awareness();

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

            let screen = GetDC(ptr::null_mut());
            let dpi = GetDeviceCaps(screen, LOGPIXELSX);
            ReleaseDC(ptr::null_mut(), screen);

            let mut rect = RECT {
                left: 0,
                top: 0,
                right: MulDiv(width, dpi, 96),
                bottom: MulDiv(height, dpi, 96),
            };
            AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, 0);

            let mut client_rect = Default::default();
            GetClientRect(GetDesktopWindow(), &mut client_rect);
            let left = (client_rect.right / 2) - ((rect.right - rect.left) / 2);
            let top = (client_rect.bottom / 2) - ((rect.bottom - rect.top) / 2);

            rect.right = rect.right - rect.left + left;
            rect.left = left;
            rect.bottom = rect.bottom - rect.top + top;
            rect.top = top;

            let mut style = WS_OVERLAPPEDWINDOW;

            if !resizable {
                style &= !(WS_SIZEBOX);
            }

            if frameless {
                style &= !(WS_SYSMENU | WS_CAPTION | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
            }

            let title = to_wstring("mshtml_webview");
            let h_wnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                style,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                HWND_DESKTOP,
                ptr::null_mut(),
                h_instance,
                ptr::null_mut(),
            );

            SetWindowLongPtrW(h_wnd, GWL_STYLE, style as _);

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

    pub(crate) fn set_color(&self, red: u8, green: u8, blue: u8, _alpha: u8) {
        unsafe {
            let brush = CreateSolidBrush(RGB(red, green, blue));
            SetClassLongPtrW(self.h_wnd, GCLP_HBRBACKGROUND, brush as _);
        }
    }
}

fn enable_dpi_awareness() -> bool {
    type FnSetThreadDpiAwarenessContext =
        extern "system" fn(dpi_context: DPI_AWARENESS_CONTEXT) -> DPI_AWARENESS_CONTEXT;

    type FnSetProcessDpiAware = extern "system" fn() -> BOOL;

    let user32 = "user32.dll";
    let user32 = to_wstring(user32);

    unsafe {
        let hmodule = GetModuleHandleW(user32.as_ptr());
        if hmodule.is_null() {
            return false;
        }

        let set_thread_dpi_awareness = CString::new("SetThreadDpiAwarenessContext").unwrap();
        let set_thread_dpi_awareness = GetProcAddress(hmodule, set_thread_dpi_awareness.as_ptr());

        if !set_thread_dpi_awareness.is_null() {
            let set_thread_dpi_awareness: FnSetThreadDpiAwarenessContext =
                std::mem::transmute(set_thread_dpi_awareness);
            if !set_thread_dpi_awareness(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE).is_null() {
                return true;
            }
        }

        let set_process_dpi_aware = CString::new("SetProcessDPIAware").unwrap();
        let set_process_dpi_aware = GetProcAddress(hmodule, set_process_dpi_aware.as_ptr());

        if set_process_dpi_aware.is_null() {
            return false;
        }

        let set_process_dpi_aware: FnSetProcessDpiAware =
            std::mem::transmute(set_process_dpi_aware);
        set_process_dpi_aware() != 0
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
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
