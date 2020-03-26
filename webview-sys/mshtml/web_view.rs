use std::{
    ffi::{OsStr, OsString},
    os::windows::ffi::OsStrExt,
    ptr::{self, NonNull},
};

use com::{co_class, interfaces::iunknown::IUnknown, ComPtr, ComRc};
use libc::c_void;
use winapi::{
    shared::{
        guiddef::{GUID, IID, IID_NULL, REFCLSID},
        minwindef::{BOOL, DWORD, FILETIME, UINT, WORD},
        ntdef::{LCID, LOCALE_SYSTEM_DEFAULT, LPWSTR, WCHAR},
        windef::{HWND, LPCRECT, LPRECT, POINT, RECT, SIZE},
        winerror::{E_FAIL, E_NOINTERFACE, E_NOTIMPL, E_PENDING, FAILED, HRESULT, S_FALSE, S_OK},
        wtypes::{VARTYPE, VT_BSTR, VT_VARIANT},
        wtypesbase::LPOLESTR,
    },
    um::{
        oaidl::{DISPID, DISPPARAMS, EXCEPINFO, VARIANT},
        objidl::{FORMATETC, SNB},
        objidlbase::STATSTG,
        oleauto::{
            SafeArrayAccessData, SafeArrayCreateVector, SafeArrayDestroy, SysAllocString,
            SysFreeString,
        },
        winuser::*,
    },
};

use crate::mshtml::interface::*;

type LPFORMATETC = *mut FORMATETC;

// "8856F961-340A-11D0-A96B-00C04FD705A2"
#[allow(non_upper_case_globals)]
const CLSID_WebBrowser: com::sys::IID = com::sys::IID {
    data1: 0x8856F961,
    data2: 0x340A,
    data3: 0x11D0,
    data4: [0xA9, 0x6B, 0x00, 0xC0, 0x4F, 0xD7, 0x05, 0xA2],
};

extern "stdcall" {
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
}

#[co_class(implements(IOleClientSite, IOleInPlaceSite, IStorage, IDocHostUIHandler))]
pub(crate) struct WebView {
    inner: Option<WebViewInner>,
}

struct WebViewInner {
    hwnd_parent: HWND,
    rect: RECT,
    ole_in_place_object: ComPtr<dyn IOleInPlaceObject>,
    web_browser: ComPtr<dyn IWebBrowser>,
    invoke_receiver: *mut ExternalInvokeReceiver, // this should be ComPtr
}

#[co_class(implements(IDispatch))]
struct ExternalInvokeReceiver {
    callback: Option<Box<dyn Fn(String)>>,
}

impl ExternalInvokeReceiver {
    fn new() -> Box<ExternalInvokeReceiver> {
        ExternalInvokeReceiver::allocate(None)
    }

    fn set_callback(&mut self, callback: Option<Box<dyn Fn(String)>>) {
        self.callback = callback;
    }

    fn invoke_callback(&self, data: String) {
        if let Some(callback) = &self.callback {
            callback(data);
        }
    }
}

impl WebView {
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

    pub(crate) fn new() -> Box<WebView> {
        WebView::allocate(None)
    }

    pub(crate) fn set_callback(&mut self, callback: Option<Box<dyn Fn(String)>>) {
        let inner = self.inner.as_ref().unwrap();
        if inner.invoke_receiver.is_null() {
            panic!("cannot set callback, invoke receiver is null");
        }

        unsafe {
            (*inner.invoke_receiver).set_callback(callback);
        }
    }

    pub(crate) fn set_rect(&self, rect: RECT) {
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

    pub(crate) fn navigate(&self, url: &str) {
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

    pub(crate) fn write(&self, document: &str) {
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

    pub(crate) fn eval(&self, js: &str) {
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

    pub(crate) fn initialize(&mut self, h_wnd: HWND, rect: RECT) {
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
            let mut hwnd_control: HWND = ptr::null_mut();
            ole_in_place_object.get_window(&mut hwnd_control);
            assert!(!hwnd_control.is_null(), "in place object hwnd is null");

            let web_browser = ioleobject
                .get_interface::<dyn IWebBrowser>()
                .expect("get interface IWebBrowser failed");

            let invoke_receiver = ExternalInvokeReceiver::new();
            let invoke_receiver = Box::into_raw(invoke_receiver);

            self.inner = Some(WebViewInner {
                hwnd_parent: h_wnd,
                rect,
                ole_in_place_object,
                web_browser,
                invoke_receiver,
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

            let self_ptr = self as *mut _;
            SetWindowLongPtrW(h_wnd, GWLP_USERDATA, self_ptr as _);
        }
    }
}

// Implementations of COM interfaces

impl IOleClientSite for WebView {
    unsafe fn save_object(&self) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn get_moniker(
        &self,
        dw_assign: DWORD,
        dw_which_moniker: DWORD,
        ppmk: *mut *mut c_void,
    ) -> HRESULT {
        // dw_assign: OLEGETMONIKER_ONLYIFTHERE = 1
        // dw_which_moniker: OLEWHICHMK_CONTAINER = 1

        if dw_assign == 1 || dw_which_moniker == 1 {
            E_FAIL
        } else {
            E_NOTIMPL
        }
    }
    unsafe fn get_container(&self, pp_container: *mut *mut c_void) -> HRESULT {
        E_NOINTERFACE
    }
    unsafe fn show_object(&self) -> HRESULT {
        S_OK
    }
    unsafe fn on_show_window(&self, show: BOOL) -> HRESULT {
        S_OK
    }
    unsafe fn request_new_object_layout(&self) -> HRESULT {
        E_NOTIMPL
    }
}

impl IOleWindow for WebView {
    unsafe fn get_window(&self, phwnd: *mut HWND) -> HRESULT {
        if self.inner.is_none() {
            *phwnd = ptr::null_mut();
            return E_PENDING;
        }

        *phwnd = self.inner.as_ref().unwrap().hwnd_parent;
        S_OK
    }
    unsafe fn context_sensitive_help(&self, f_enter_mode: BOOL) -> HRESULT {
        E_NOTIMPL
    }
}

impl IOleInPlaceSite for WebView {
    unsafe fn can_in_place_activate(&self) -> HRESULT {
        S_OK
    }
    unsafe fn on_in_place_activate(&self) -> HRESULT {
        S_OK
    }
    unsafe fn on_ui_activate(&self) -> HRESULT {
        S_OK
    }
    unsafe fn get_window_context(
        &self,
        pp_frame: *mut *mut c_void,
        pp_doc: *mut *mut c_void,
        lprc_pos_rect: LPRECT,
        lprc_clip_rect: LPRECT,
        lp_frame_info: *mut OLEINPLACEFRAMEINFO,
    ) -> HRESULT {
        *pp_frame = ptr::null_mut();
        *pp_doc = ptr::null_mut();
        *lprc_pos_rect = self.inner.as_ref().unwrap().rect;
        *lprc_clip_rect = *lprc_pos_rect;

        (*lp_frame_info).fMDIApp = 0;
        (*lp_frame_info).hwndFrame = self.inner.as_ref().unwrap().hwnd_parent;
        (*lp_frame_info).haccel = ptr::null_mut();
        (*lp_frame_info).cAccelEntries = 0;
        S_OK
    }
    unsafe fn scroll(&self, scroll_extant: SIZE) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn on_ui_deactivate(&self, f_undoable: BOOL) -> HRESULT {
        S_OK
    }
    unsafe fn on_in_place_deactivate(&self) -> HRESULT {
        S_OK
    }
    unsafe fn discard_undo_state(&self) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn deactivate_and_undo(&self) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn on_pos_rect_change(&self, lprc_post_rect: LPRECT) -> HRESULT {
        E_NOTIMPL
    }
}

impl IStorage for WebView {
    unsafe fn create_stream(
        &self,
        pwcs_name: *const WCHAR,
        grf_mode: DWORD,
        reserved1: DWORD,
        reserved2: DWORD,
        ppstm: *mut *mut c_void,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn open_stream(
        &self,
        pwcs_name: *const WCHAR,
        reserved1: *mut c_void,
        grf_mode: DWORD,
        reserved2: DWORD,
        ppstm: *mut *mut c_void,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn create_storage(
        &self,
        pwcs_name: *const WCHAR,
        grf_mode: DWORD,
        reserved1: DWORD,
        reserved2: DWORD,
        ppstg: *mut *mut c_void,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn open_storage(
        &self,
        pwcs_name: *const WCHAR,
        pstg_priority: *mut c_void,
        grf_mode: DWORD,
        snb_exclude: SNB,
        reserved: DWORD,
        ppstg: *mut *mut c_void,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn copy_to(
        &self,
        ciid_exclude: DWORD,
        rgiid_exclude: *const IID,
        snb_exclude: SNB,
        pstg_dest: *mut c_void,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn move_element_to(
        &self,
        pwcs_name: *const WCHAR,
        pstg_dest: *mut c_void,
        pwcs_new_name: *const WCHAR,
        grf_flags: DWORD,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn commit(&self, grf_commit_flags: DWORD) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn revert(&self) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn enum_elements(
        &self,
        reserved1: DWORD,
        reserved2: *mut c_void,
        reserved3: DWORD,
        ppenum: *mut *mut c_void,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn destroy_element(&self, pwcs_name: *const WCHAR) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn rename_element(
        &self,
        pwcs_old_name: *const WCHAR,
        pwcs_new_name: *const WCHAR,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn set_element_times(
        &self,
        pwcs_name: *const WCHAR,
        pctime: *const FILETIME,
        patime: *const FILETIME,
        pmtime: *const FILETIME,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn set_class(&self, clsid: REFCLSID) -> HRESULT {
        S_OK
    }
    unsafe fn set_state_bits(&self, grf_state_bits: DWORD, grf_mask: DWORD) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn stat(&self, pstatstg: *mut STATSTG, grf_stat_flag: DWORD) -> HRESULT {
        E_NOTIMPL
    }
}

impl IDocHostUIHandler for WebView {
    unsafe fn show_context_menu(
        &self,
        dw_id: DWORD,
        ppt: *mut POINT,
        pcmdt_reserved: *mut c_void, /*IUnknown*/
        pdisp_reserved: *mut c_void, /*IDispatch*/
    ) -> HRESULT {
        S_OK
    }
    unsafe fn get_host_info(&self, p_info: *mut c_void /*DOCHOSTUIINFO*/) -> HRESULT {
        E_NOTIMPL
    }
    unsafe fn show_ui(
        &self,
        dw_id: DWORD,
        p_active_object: *mut c_void,  /*IOleInPlaceActiveObject*/
        p_command_target: *mut c_void, /*IOleCommandTarget*/
        p_frame: *mut c_void,          /*IOleInPlaceFrame*/
        p_doc: *mut c_void,            /*IOleInPlaceUIWindow*/
    ) -> HRESULT {
        S_OK
    }
    unsafe fn hide_ui(&self) -> HRESULT {
        S_OK
    }
    unsafe fn update_ui(&self) -> HRESULT {
        S_OK
    }
    unsafe fn enable_modeless(&self, f_enable: BOOL) -> HRESULT {
        S_OK
    }
    unsafe fn on_doc_window_activate(&self, f_activate: BOOL) -> HRESULT {
        S_OK
    }
    unsafe fn on_frame_window_activate(&self, f_activate: BOOL) -> HRESULT {
        S_OK
    }
    unsafe fn resize_border(
        &self,
        prc_border: LPCRECT,
        p_ui_window: *mut c_void, /*IOleInPlaceUIWindow*/
        f_rame_window: BOOL,
    ) -> HRESULT {
        S_OK
    }
    unsafe fn translate_accelerator(
        &self,
        lp_msg: LPMSG,
        pguid_cmd_group: *const GUID,
        n_cmd_id: DWORD,
    ) -> HRESULT {
        S_FALSE
    }
    unsafe fn get_option_key_path(&self, pch_key: *mut LPOLESTR, dw: DWORD) -> HRESULT {
        S_FALSE
    }
    unsafe fn get_drop_target(
        &self,
        p_drop_target: *mut c_void,       /*IDropTarget*/
        pp_drop_target: *mut *mut c_void, /*IDropTarget*/
    ) -> HRESULT {
        S_FALSE
    }
    unsafe fn get_external(&self, pp_dispatch: *mut *mut c_void /*IDispatch*/) -> HRESULT {
        let inner = self.inner.as_ref().unwrap();
        (*inner.invoke_receiver).add_ref();
        *pp_dispatch = inner.invoke_receiver as _;
        S_OK
    }
    unsafe fn translate_url(
        &self,
        dw_translate: DWORD,
        pch_url_in: LPWSTR,
        ppch_url_out: *mut LPWSTR,
    ) -> HRESULT {
        *ppch_url_out = ptr::null_mut();
        S_FALSE
    }
    unsafe fn filter_data_object(
        &self,
        p_do: *mut c_void,           /*IDataObject*/
        pp_do_ret: *mut *mut c_void, /*IDataObject*/
    ) -> HRESULT {
        *pp_do_ret = ptr::null_mut();
        S_FALSE
    }
}

unsafe fn from_wstring(ptr: *const u16) -> OsString {
    use std::os::windows::ffi::OsStringExt;

    let len = (0..).take_while(|&i| *ptr.offset(i) != 0).count();
    let slice = std::slice::from_raw_parts(ptr, len);

    OsString::from_wide(slice)
}

unsafe fn from_utf16(ptr: *const u16) -> String {
    let len = (0..).take_while(|&i| *ptr.offset(i) != 0).count();
    let slice = std::slice::from_raw_parts(ptr, len);
    String::from_utf16(slice).expect("invalid utf16")
}

const WEBVIEW_JS_INVOKE_ID: DISPID = 0x1000;

impl IDispatch for ExternalInvokeReceiver {
    unsafe fn get_type_info_count(&self, pctinfo: *mut UINT) -> HRESULT {
        S_OK
    }
    unsafe fn get_type_info(
        &self,
        i_ti_info: UINT,
        icid: LCID,
        pp_ti_info: *mut *mut c_void,
    ) -> HRESULT {
        S_OK
    }
    unsafe fn get_ids_of_names(
        &self,
        riid: *const IID,
        rgsz_names: *mut LPOLESTR,
        c_names: UINT,
        lcid: LCID,
        rg_disp_id: *mut DISPID,
    ) -> HRESULT {
        let names = std::slice::from_raw_parts(rgsz_names, c_names as _);
        if names.len() == 1 {
            let name = from_wstring(names[0]);
            if name == "invoke" {
                // map the invoke function on external object to this id
                *rg_disp_id.offset(0) = WEBVIEW_JS_INVOKE_ID;
                return S_OK;
            }
        }

        S_FALSE
    }
    unsafe fn invoke(
        &self,
        disp_id_member: DISPID,
        riid: *const IID,
        lcid: LCID,
        w_flags: WORD,
        p_disp_params: *mut DISPPARAMS,
        p_var_result: *mut VARIANT,
        p_excep_info: *mut EXCEPINFO,
        pu_arg_err: *mut UINT,
    ) -> HRESULT {
        // first we check if the message the webview is trying to
        // invoke is the method we gave it in get_ids_of_names
        // through the custom id we specified
        if disp_id_member == WEBVIEW_JS_INVOKE_ID {
            let params = NonNull::new(p_disp_params).expect("p_disp_params is null");
            let params = params.as_ref();
            let vargs = std::slice::from_raw_parts(params.rgvarg, params.cArgs as _);

            // we only handle invoke function which has only one positional argument
            // and the argument needs to be string
            if vargs.len() == 1 {
                let varg = &vargs[0];

                // check if the argument is string,
                // convert it to String from utf16
                // and pass it further
                if varg.n1.n2().vt == VT_BSTR as VARTYPE {
                    let arg = *varg.n1.n2().n3.bstrVal();
                    let arg = from_utf16(arg);
                    self.invoke_callback(arg);
                    return S_OK;
                }
            }
        }

        S_FALSE
    }
}

fn to_wstring(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect()
}

// unsafe fn from_wstring(wide: *const u16) -> OsString {
//     assert!(!wide.is_null());
//     for i in 0.. {
//         if *wide.offset(i) == 0 {
//             return OsStringExt::from_wide(std::slice::from_raw_parts(wide, i as usize));
//         }
//     }
//     unreachable!()
// }
