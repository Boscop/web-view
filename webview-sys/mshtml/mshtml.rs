#![cfg(target_os = "windows")]
#![allow(unused_variables)]

use std::ffi::{CStr, OsStr};
use std::ffi::{CString, OsString};
use std::mem;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::ptr;

use com::{co_class, interfaces::IUnknown, ComPtr, ComRc};
use libc::{c_char, c_int, c_void};
use percent_encoding::percent_decode_str;
use winapi::shared::guiddef::IID_NULL;
use winapi::shared::minwindef::{BOOL, DWORD, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::ntdef::LOCALE_SYSTEM_DEFAULT;
use winapi::shared::winerror::{FAILED, HRESULT};
use winapi::shared::wtypes::{VT_BSTR, VT_VARIANT};
use winapi::shared::{ntdef, windef, winerror};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::libloaderapi::{GetModuleHandleW, GetProcAddress};
use winapi::um::oaidl::{DISPID, DISPPARAMS, VARIANT};
use winapi::um::objidl::FORMATETC;
use winapi::um::ole2::OleInitialize;
use winapi::um::oleauto::{
    SafeArrayAccessData, SafeArrayCreateVector, SafeArrayDestroy, SysAllocString, SysFreeString,
};
use winapi::um::winbase::MulDiv;
use winapi::um::wingdi::{GetDeviceCaps, LOGPIXELSX};
use winapi::um::winuser::*;
use winreg::{enums, RegKey};

mod interface;
use interface::*;

const WM_WEBVIEW_DISPATCH: u32 = WM_APP + 1;

// "8856F961-340A-11D0-A96B-00C04FD705A2"
#[allow(non_upper_case_globals)]
const CLSID_WebBrowser: com::sys::IID = com::sys::IID {
    data1: 0x8856F961,
    data2: 0x340A,
    data3: 0x11D0,
    data4: [0xA9, 0x6B, 0x00, 0xC0, 0x4F, 0xD7, 0x05, 0xA2],
};

type LPFORMATETC = *mut FORMATETC;

type ExternalInvokeCallback = extern "C" fn(webview: *mut WebView, arg: *const c_char);
type ErasedDispatchFn = extern "C" fn(webview: *mut WebView, arg: *mut c_void);

extern "system" {
    // winapi does not have these functions defined, we need to declare it ourselves

    fn OleCreate(
        rclsid: *const com::sys::IID,
        riid: *const com::sys::IID,
        renderopt: DWORD,
        pFormatEtc: LPFORMATETC,
        p_client_size: *mut c_void,
        p_str: *mut c_void,
        ppv_obj: *mut *mut c_void,
    ) -> HRESULT;

    fn OleSetContainedObject(p_unknown: *mut c_void, f_contained: BOOL) -> HRESULT;

    fn OleUninitialize();
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
    is_fullscreen: BOOL,
    saved_style: ntdef::LONG,
    saved_ex_style: ntdef::LONG,
    saved_rect: windef::RECT,
    browser: Option<Box<WebBrowser>>,
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
        let result = OleInitialize(ptr::null_mut());
        if result != winerror::S_OK && result != winerror::S_FALSE {
            return ptr::null_mut();
        }

        let h_instance = GetModuleHandleW(ptr::null_mut()); // check A vs W
        if h_instance.is_null() {
            return ptr::null_mut();
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
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        if RegisterClassW(&class) == 0 {
            // ignore the "Class already exists" error for multiple windows
            if GetLastError() as u32 != 1410 {
                eprintln!("Unable to register class, error {}", GetLastError() as u32);

                OleUninitialize();
                return ptr::null_mut();
            }
        }

        // Return value not checked. If this function fails, simply continue without
        // high DPI support.
        let _ = enable_dpi_awareness();
        let mut style = WS_OVERLAPPEDWINDOW;
        if resizable == 0 {
            style &= !WS_SIZEBOX;
        }

        if frameless != 0 {
            style &= !(WS_SYSMENU | WS_CAPTION | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
        }

        // Get DPI.
        let screen = GetDC(ptr::null_mut());
        let dpi = GetDeviceCaps(screen, LOGPIXELSX);
        ReleaseDC(ptr::null_mut(), screen);

        let mut rect = windef::RECT {
            left: 0,
            right: MulDiv(width, dpi, 96),
            top: 0,
            bottom: MulDiv(height, dpi, 96),
        };

        AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, 0);

        let mut client_rect = windef::RECT::default();
        GetClientRect(GetDesktopWindow(), &mut client_rect);
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
            is_fullscreen: 0,
            saved_style: 0,
            saved_ex_style: 0,
            saved_rect: rect,
            browser: None,
        });

        let webview_ptr = Box::into_raw(webview);

        let handle = CreateWindowExW(
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
            webview_ptr as *mut winapi::ctypes::c_void,
        );

        if handle.is_null() {
            eprintln!("Unable to create window, error {}", GetLastError() as u32);

            let _ = Box::from_raw(webview_ptr); // properly drop webview on failure
            OleUninitialize();
            return ptr::null_mut();
        }

        (*webview_ptr).hwnd = handle;

        let mut browser = WebBrowser::new();
        browser.initialize(handle, rect);

        let url = CStr::from_ptr(url);
        let url = url.to_str().expect("could not convert url");

        if url.starts_with("data:text/html,") {
            let url = &url["data:text/html,".len()..];
            let url = percent_decode_str(url).decode_utf8().unwrap();
            browser.navigate("about:blank");
            browser.write(&url);
        } else {
            browser.navigate(url);
        }

        (*webview_ptr).browser = Some(browser);

        SetWindowLongPtrW(handle, GWLP_USERDATA, std::mem::transmute(webview_ptr));

        if frameless != 0 {
            SetWindowLongPtrW(handle, GWL_STYLE, style as _);
        }

        // DisplayHTMLPage(webview_ptr);

        ShowWindow(handle, SW_SHOWDEFAULT);
        UpdateWindow(handle);
        SetFocus(handle);

        webview_ptr
    }
}

// WEBVIEW_API int webview_loop(webview_t w, int blocking) {
//     struct mshtml_webview* wv = (struct mshtml_webview*)w;
//     MSG msg;
//     if (blocking) {
//       if (GetMessage(&msg, 0, 0, 0) < 0) return 0;
//     } else {
//       if (PeekMessage(&msg, 0, 0, 0, PM_REMOVE) == 0) return 0;
//     }
//     switch (msg.message) {
//     case WM_QUIT:
//       return -1;
//     case WM_COMMAND:
//     case WM_KEYDOWN:
//     case WM_KEYUP: {
//       HRESULT r = S_OK;
//       IWebBrowser2 *webBrowser2;
//       IOleObject *browser = *wv->browser;
//       if (browser->lpVtbl->QueryInterface(browser, iid_unref(&IID_IWebBrowser2),
//                                           (void **)&webBrowser2) == S_OK) {
//         IOleInPlaceActiveObject *pIOIPAO;
//         if (browser->lpVtbl->QueryInterface(
//                 browser, iid_unref(&IID_IOleInPlaceActiveObject),
//                 (void **)&pIOIPAO) == S_OK) {
//           r = pIOIPAO->lpVtbl->TranslateAccelerator(pIOIPAO, &msg);
//           pIOIPAO->lpVtbl->Release(pIOIPAO);
//         }
//         webBrowser2->lpVtbl->Release(webBrowser2);
//       }
//       if (r != S_FALSE) {
//         break;
//       }
//     }
//     default:
//       TranslateMessage(&msg);
//       DispatchMessage(&msg);
//     }
//     return 0;
//   }

// LRESULT CALLBACK wndproc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam) {
//     struct mshtml_webview *wv = (struct mshtml_webview *)GetWindowLongPtr(hwnd, GWLP_USERDATA);
//     switch (uMsg) {
//     case WM_CREATE:
//       wv = (struct mshtml_webview *)((CREATESTRUCT *)lParam)->lpCreateParams;
//       wv->hwnd = hwnd;
//       return EmbedBrowserObject(wv);
//     case WM_DESTROY:
//       UnEmbedBrowserObject(wv);
//       PostQuitMessage(0);
//       return TRUE;
//     case WM_SIZE: {
//       IWebBrowser2 *webBrowser2;
//       IOleObject *browser = *wv->browser;
//       if (browser->lpVtbl->QueryInterface(browser, iid_unref(&IID_IWebBrowser2),
//                                           (void **)&webBrowser2) == S_OK) {
//         RECT rect;
//         GetClientRect(hwnd, &rect);
//         webBrowser2->lpVtbl->put_Width(webBrowser2, rect.right);
//         webBrowser2->lpVtbl->put_Height(webBrowser2, rect.bottom);
//       }
//       return TRUE;
//     }
//     case WM_WEBVIEW_DISPATCH: {
//       webview_dispatch_fn f = (webview_dispatch_fn)wParam;
//       void *arg = (void *)lParam;
//       (*f)(wv, arg);
//       return TRUE;
//     }
//     }
//     return DefWindowProc(hwnd, uMsg, wParam, lParam);
//   }

unsafe extern "system" fn wndproc(
    window: windef::HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => DefWindowProcW(window, msg, wparam, lparam),
        WM_DESTROY => {
            PostQuitMessage(0);
            return 1;
        }
        WM_SIZE => {
            let webview_ptr: *mut WebView =
                std::mem::transmute(GetWindowLongPtrW(window, GWLP_USERDATA));
            if webview_ptr.is_null() {
                return 1;
            }

            let mut rect: windef::RECT = Default::default();
            GetClientRect(window, &mut rect);
            (*webview_ptr).browser.as_mut().unwrap().set_rect(rect);
            return 0;
        }
        WM_WEBVIEW_DISPATCH => 0,
        _ => DefWindowProcW(window, msg, wparam, lparam),
    }
}

#[no_mangle]
unsafe extern "C" fn webview_loop(_webview: *mut WebView, blocking: c_int) -> c_int {
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
unsafe extern "C" fn webview_eval(webview: *mut WebView, js: *const c_char) -> c_int {
    let js = CStr::from_ptr(js);
    let js = js.to_str().expect("js is not valid utf8");
    println!("eval {}", js);
    (*webview).browser.as_ref().unwrap().eval(js);
    return 0;
}

#[no_mangle]
unsafe extern "C" fn webview_exit(webview: *mut WebView) {
    DestroyWindow((*webview).hwnd);
    OleUninitialize();
}

#[no_mangle]
unsafe extern "C" fn webview_free(webview: *mut WebView) {
    let _ = Box::from_raw(webview);
}

#[no_mangle]
unsafe extern "C" fn webview_get_user_data(webview: *mut WebView) -> *mut c_void {
    (*webview).userdata
}

#[no_mangle]
unsafe extern "C" fn webview_dispatch(
    webview: *mut WebView,
    f: Option<ErasedDispatchFn>,
    arg: *mut c_void,
) {
    PostMessageW(
        (*webview).hwnd,
        WM_WEBVIEW_DISPATCH,
        mem::transmute(f),
        arg as _,
    );
}

fn enable_dpi_awareness() -> bool {
    type FnSetThreadDpiAwarenessContext = extern "system" fn(
        dpi_context: windef::DPI_AWARENESS_CONTEXT,
    ) -> windef::DPI_AWARENESS_CONTEXT;

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
                mem::transmute(set_thread_dpi_awareness);
            if !set_thread_dpi_awareness(windef::DPI_AWARENESS_CONTEXT_SYSTEM_AWARE).is_null() {
                return true;
            }
        }

        let set_process_dpi_aware = CString::new("SetProcessDPIAware").unwrap();
        let set_process_dpi_aware = GetProcAddress(hmodule, set_process_dpi_aware.as_ptr());

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

#[co_class(implements(IOleClientSite, IOleInPlaceSite, IStorage))]
struct WebBrowser {
    inner: Option<WebBrowserInner>,
}

struct WebBrowserInner {
    hwnd_parent: windef::HWND,
    rect: windef::RECT,
    ole_in_place_object: ComPtr<dyn IOleInPlaceObject>,
    web_browser: ComPtr<dyn IWebBrowser>,
}

impl WebBrowser {
    /// A safe version of `QueryInterface`. If the backing CoClass implements the
    /// interface `I` then a `Some` containing an `ComRc` pointing to that
    /// interface will be returned otherwise `None` will be returned.
    fn get_interface<I: com::ComInterface + ?Sized>(&self) -> Option<ComPtr<I>> {
        let mut ppv = std::ptr::null_mut::<c_void>();
        let hr = unsafe { self.query_interface(&I::IID as *const com::sys::IID, &mut ppv) };
        if FAILED(hr) {
            assert!(
                hr == com::sys::E_NOINTERFACE || hr == com::sys::E_POINTER,
                "QueryInterface returned non-standard error"
            );
            return None;
        }
        assert!(!ppv.is_null(), "The pointer to the interface returned from a successful call to QueryInterface was null");
        Some(unsafe { ComPtr::new(ppv as *mut *mut _) })
    }

    fn new() -> Box<WebBrowser> {
        WebBrowser::allocate(None)
    }

    fn set_rect(&self, rect: windef::RECT) {
        if self.inner.is_none() {
            return;
        }

        unsafe {
            self.inner
                .as_ref()
                .unwrap()
                .ole_in_place_object
                .set_object_rects(&rect, &rect);
        }
    }

    fn navigate(&self, url: &str) {
        println!("navigating to {}", url);
        let mut wstring = to_wstring(url);
        unsafe {
            self.inner.as_ref().unwrap().web_browser.navigate(
                wstring.as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
        }
    }

    fn write(&self, document: &str) {
        println!("writing {}", document);
        let inner = self.inner.as_ref().unwrap();
        unsafe {
            let mut document_dispatch = ptr::null_mut::<c_void>();
            let h_result = inner.web_browser.get_document(&mut document_dispatch);
            if FAILED(h_result) || document_dispatch.is_null() {
                panic!("get_document failed {}", h_result);
            }

            let document_dispatch =
                ComRc::<dyn IDispatch>::from_raw(document_dispatch as *mut *mut _);

            let html_document2 = document_dispatch
                .get_interface::<dyn IHTMLDocument2>()
                .expect("cannot get IHTMLDocument2 interface");

            let safe_array = SafeArrayCreateVector(VT_VARIANT as _, 0, 1);
            if safe_array.is_null() {
                panic!("SafeArrayCreate failed");
            }
            let mut data: [*mut VARIANT; 1] = [ptr::null_mut()];
            let h_result = SafeArrayAccessData(safe_array, data.as_mut_ptr() as _);
            if FAILED(h_result) {
                panic!("SafeArrayAccessData failed");
            }

            let document = to_wstring(document);
            let document = SysAllocString(document.as_ptr());

            if document.is_null() {
                panic!("SysAllocString document failed");
            }
            let variant = &mut (*data[0]);
            variant.n1.n2_mut().vt = VT_BSTR as _;
            *variant.n1.n2_mut().n3.bstrVal_mut() = document;
            if FAILED(html_document2.write(safe_array)) {
                panic!("html_document2.write() failed");
            }
            if FAILED(html_document2.close()) {
                panic!("html_document2.close() failed");
            }

            SysFreeString(document);
            SafeArrayDestroy(safe_array);
        }
    }

    fn eval(&self, js: &str) {
        let inner = self.inner.as_ref().unwrap();
        unsafe {
            let mut document_dispatch = ptr::null_mut::<c_void>();
            let h_result = inner.web_browser.get_document(&mut document_dispatch);
            if FAILED(h_result) || document_dispatch.is_null() {
                panic!("get_document failed {}", h_result);
            }

            let document_dispatch =
                ComRc::<dyn IDispatch>::from_raw(document_dispatch as *mut *mut _);

            let html_document = document_dispatch
                .get_interface::<dyn IHTMLDocument>()
                .expect("cannot get IHTMLDocument interface");

            let mut script_dispatch = ptr::null_mut::<c_void>();
            let h_result = html_document.get_script(&mut script_dispatch);
            if FAILED(h_result) || script_dispatch.is_null() {
                panic!("get_script failed {}", h_result);
            }

            let script_dispatch = ComRc::<dyn IDispatch>::from_raw(script_dispatch as *mut *mut _);

            let mut eval_name = to_wstring("eval");
            let mut names = [eval_name.as_mut_ptr()];
            let mut disp_ids = [DISPID::default()];
            assert_eq!(names.len(), disp_ids.len());

            // get_ids_of_names can fail if there is no loaded document
            // should we hande this by loading empty document?

            let h_result = script_dispatch.get_ids_of_names(
                &IID_NULL,
                names.as_mut_ptr(),
                names.len() as _,
                LOCALE_SYSTEM_DEFAULT,
                disp_ids.as_mut_ptr(),
            );
            if FAILED(h_result) {
                panic!("get_ids_of_names failed {}", h_result);
            }

            let js = to_wstring(js);

            // we need to free this later
            // with SysFreeString
            let js = SysAllocString(js.as_ptr());
            let mut varg = VARIANT::default();
            varg.n1.n2_mut().vt = VT_BSTR as _;

            // we cannot pass regular BSTR here,
            // it needs to be allocated with SysAllocString
            // which also allocates inner data for
            // the specific BSTR such as refcount
            *varg.n1.n2_mut().n3.bstrVal_mut() = js;

            let mut args = [varg];
            let mut disp_params = DISPPARAMS {
                rgvarg: args.as_mut_ptr(),          // array of positional arguments
                rgdispidNamedArgs: ptr::null_mut(), // array of dispids for named args
                cArgs: args.len() as _,             // number of position arguments
                cNamedArgs: 0,                      // number of named args - none
            };
            let h_result = script_dispatch.invoke(
                disp_ids[0],
                &IID_NULL,
                0,
                1,
                &mut disp_params,
                ptr::null_mut(), // should we implement result?
                ptr::null_mut(),
                ptr::null_mut(),
            );

            SysFreeString(js);

            // this should be catchable by user,
            // it does not always have to be irrecoverable error
            if FAILED(h_result) {
                panic!("invoke failed {}", h_result);
            }
        }
    }

    fn initialize(&mut self, h_wnd: windef::HWND, rect: windef::RECT) {
        unsafe {
            let iole_client_site = self
                .get_interface::<dyn IOleClientSite>()
                .expect("iole_client_site query failed");

            let istorage = self
                .get_interface::<dyn IStorage>()
                .expect("istorage query failed");

            let mut ioleobject_ptr = ptr::null_mut::<c_void>();
            let hresult = OleCreate(
                &CLSID_WebBrowser,
                &<dyn IOleObject as com::ComInterface>::IID,
                1,
                ptr::null_mut(),
                iole_client_site.as_raw() as _,
                istorage.as_raw() as _,
                &mut ioleobject_ptr,
            );

            if FAILED(hresult) {
                panic!("cannot create WebBrowser ole object");
            }

            let ioleobject = ComPtr::<dyn IOleObject>::new(ioleobject_ptr as *mut *mut _);
            let hresult = OleSetContainedObject(ioleobject.as_raw() as _, 1);

            if FAILED(hresult) {
                panic!("OleSetContainedObject() failed");
            }

            let ole_in_place_object = ioleobject
                .get_interface::<dyn IOleInPlaceObject>()
                .expect("cannot query ole_in_place_object");

            ole_in_place_object.set_object_rects(&rect, &rect);
            let mut hwnd_control: windef::HWND = ptr::null_mut();
            ole_in_place_object.get_window(&mut hwnd_control);
            assert!(!hwnd_control.is_null(), "in place object hwnd is null");

            let web_browser = ioleobject
                .get_interface::<dyn IWebBrowser>()
                .expect("get interface IWebBrowser failed");

            self.inner = Some(WebBrowserInner {
                hwnd_parent: h_wnd,
                rect,
                ole_in_place_object,
                web_browser,
            });

            let hresult = ioleobject.do_verb(
                -5,
                ptr::null_mut(),
                iole_client_site.as_raw() as _,
                -1,
                h_wnd,
                &rect,
            );

            if FAILED(hresult) {
                panic!("ioleobject.do_verb() failed");
            }
        }
    }
}
