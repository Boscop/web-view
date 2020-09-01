#include "webview.h"

#define CINTERFACE
#include <windows.h>

#include <commctrl.h>
#include <exdisp.h>
#include <mshtmhst.h>
#include <mshtml.h>
#include <shobjidl.h>

#include <stdio.h>

#pragma comment(lib, "ole32.lib")
#pragma comment(lib, "comctl32.lib")
#pragma comment(lib, "oleaut32.lib")
#pragma comment(lib, "uuid.lib")
#pragma comment(lib, "gdi32.lib")
#pragma comment(lib, "user32.lib")

// For GCC.
#ifndef DPI_AWARENESS_CONTEXT_SYSTEM_AWARE
DECLARE_HANDLE(DPI_AWARENESS_CONTEXT);
#define DPI_AWARENESS_CONTEXT_SYSTEM_AWARE ((DPI_AWARENESS_CONTEXT)-2)
#endif

struct mshtml_webview {
  const char *url;
  int width;
  int height;
  int resizable;
  int debug;
  int frameless;
  int min_width;
  int min_height;
  webview_external_invoke_cb_t external_invoke_cb;
  void *userdata;
  HWND hwnd;
  IOleObject **browser;
  BOOL is_fullscreen;
  BOOL is_maximized;
  DWORD saved_style;
  DWORD saved_ex_style;
  RECT saved_rect;
};

LRESULT CALLBACK wndproc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam);
static BOOL EnableDpiAwareness();
static int DisplayHTMLPage(struct mshtml_webview *wv);

WEBVIEW_API void webview_free(webview_t w) {
	free(w);
}

WEBVIEW_API void* webview_get_user_data(webview_t w) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
	return wv->userdata;
}

static inline BSTR webview_to_bstr(const char *s) {
  DWORD size = MultiByteToWideChar(CP_UTF8, 0, s, -1, 0, 0);
  BSTR bs = SysAllocStringLen(0, size);
  if (bs == NULL) {
    return NULL;
  }
  MultiByteToWideChar(CP_UTF8, 0, s, -1, bs, size);
  return bs;
}

#define WEBVIEW_KEY_FEATURE_BROWSER_EMULATION                                  \
  L"Software\\Microsoft\\Internet "                                            \
   "Explorer\\Main\\FeatureControl\\FEATURE_BROWSER_EMULATION"

static int webview_fix_ie_compat_mode() {
  HKEY hKey;
  DWORD ie_version = 11000;
  WCHAR appname[MAX_PATH + 1];
  WCHAR *p;
  if (GetModuleFileNameW(NULL, appname, MAX_PATH + 1) == 0) {
    return -1;
  }
  for (p = &appname[wcslen(appname) - 1]; p != appname && *p != L'\\'; p--) {
  }
  p++;
  if (RegCreateKeyW(HKEY_CURRENT_USER, WEBVIEW_KEY_FEATURE_BROWSER_EMULATION,
                    &hKey) != ERROR_SUCCESS) {
    return -1;
  }
  if (RegSetValueExW(hKey, p, 0, REG_DWORD, (BYTE *)&ie_version,
                     sizeof(ie_version)) != ERROR_SUCCESS) {
    RegCloseKey(hKey);
    return -1;
  }
  RegCloseKey(hKey);
  return 0;
}

static const TCHAR *classname = "WebView";

WEBVIEW_API webview_t webview_new(
  const char* title, const char* url, int width, int height, int resizable, int debug, 
  int frameless, int visible, int min_width, int min_height, webview_external_invoke_cb_t external_invoke_cb, void* userdata) {

  if (webview_fix_ie_compat_mode() < 0) {
    return NULL;
  }

  HINSTANCE hInstance = GetModuleHandle(NULL);
  if (hInstance == NULL) {
    return NULL;
  }

  HRESULT oleInitCode = OleInitialize(NULL);
  if (oleInitCode != S_OK && oleInitCode != S_FALSE) {
    return NULL;
  }

  // Return value not checked. If this function fails, simply continue without
  // high DPI support.
  EnableDpiAwareness();

  HICON winresIcon = (HICON)LoadImage(
      hInstance, 
      (LPWSTR)(1), 
      IMAGE_ICON, 
      0, 
      0,
      LR_DEFAULTSIZE
  );
  
  WNDCLASSEX wc;
  ZeroMemory(&wc, sizeof(WNDCLASSEX));
  wc.cbSize = sizeof(WNDCLASSEX);
  wc.hInstance = hInstance;
  wc.lpfnWndProc = wndproc;
  wc.lpszClassName = classname;
  wc.hIcon = winresIcon;
  RegisterClassEx(&wc);

  DWORD style = WS_OVERLAPPEDWINDOW;
  if (!resizable) {
      style &= ~(WS_SIZEBOX);
  }
  if (frameless) {
    style &= ~(WS_SYSMENU | WS_CAPTION | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
  }

  // Get DPI.
  HDC screen = GetDC(0);
  int DPI = GetDeviceCaps(screen, LOGPIXELSX);
  ReleaseDC(0, screen);

  RECT rect;
  rect.left = 0;
  rect.top = 0;
  rect.right = MulDiv(width, DPI, 96);
  rect.bottom = MulDiv(height, DPI, 96);
  AdjustWindowRect(&rect, WS_OVERLAPPEDWINDOW, 0);

  RECT clientRect;
  GetClientRect(GetDesktopWindow(), &clientRect);
  int left = (clientRect.right / 2) - ((rect.right - rect.left) / 2);
  int top = (clientRect.bottom / 2) - ((rect.bottom - rect.top) / 2);
  rect.right = rect.right - rect.left + left;
  rect.left = left;
  rect.bottom = rect.bottom - rect.top + top;
  rect.top = top;

  BSTR window_title = webview_to_bstr(title);
  struct mshtml_webview* wv = (struct mshtml_webview*)calloc(1, sizeof(*wv));

  wv->width = width;
  wv->height = height;
  wv->url = url;
  wv->resizable = resizable;
  wv->debug = debug;
  wv->frameless = frameless;
  wv->min_width = min_width;
  wv->min_height = min_height;
  wv->external_invoke_cb = external_invoke_cb;
  wv->userdata = userdata;
  wv->hwnd =
      CreateWindowEx(0, classname, window_title, style, rect.left, rect.top,
                     rect.right - rect.left, rect.bottom - rect.top,
                     HWND_DESKTOP, NULL, hInstance, (void *)wv);

  SysFreeString(window_title);
  if (wv->hwnd == 0) {
    webview_free(wv);
    OleUninitialize();
    return NULL;
  }

  SetWindowLongPtr(wv->hwnd, GWLP_USERDATA, (LONG_PTR)wv);

  if (wv->frameless) {
    SetWindowLongPtr(wv->hwnd, GWL_STYLE, style);
  }

  DisplayHTMLPage(wv);

  ShowWindow(wv->hwnd, visible ? SW_SHOWDEFAULT : SW_HIDE);
  UpdateWindow(wv->hwnd);
  SetFocus(wv->hwnd);

  return wv;
}

#define WM_WEBVIEW_DISPATCH (WM_APP + 1)

typedef struct {
  IOleInPlaceFrame frame;
  HWND window;
} _IOleInPlaceFrameEx;

typedef struct {
  IOleInPlaceSite inplace;
  _IOleInPlaceFrameEx frame;
} _IOleInPlaceSiteEx;

typedef struct {
  IDocHostUIHandler ui;
} _IDocHostUIHandlerEx;

typedef struct {
  IInternetSecurityManager mgr;
} _IInternetSecurityManagerEx;

typedef struct {
  IServiceProvider provider;
  _IInternetSecurityManagerEx mgr;
} _IServiceProviderEx;

typedef struct {
  IOleClientSite client;
  _IOleInPlaceSiteEx inplace;
  _IDocHostUIHandlerEx ui;
  IDispatch external;
  _IServiceProviderEx provider;
} _IOleClientSiteEx;

typedef DPI_AWARENESS_CONTEXT (WINAPI *FnSetThreadDpiAwarenessContext)(DPI_AWARENESS_CONTEXT);
typedef BOOL (WINAPI *FnSetProcessDPIAware)();

#ifdef __cplusplus
#define iid_ref(x) &(x)
#define iid_unref(x) *(x)
#else
#define iid_ref(x) (x)
#define iid_unref(x) (x)
#endif

static inline WCHAR *webview_to_utf16(const char *s) {
  DWORD size = MultiByteToWideChar(CP_UTF8, 0, s, -1, 0, 0);
  WCHAR *ws = (WCHAR *)GlobalAlloc(GMEM_FIXED, sizeof(WCHAR) * size);
  if (ws == NULL) {
    return NULL;
  }
  MultiByteToWideChar(CP_UTF8, 0, s, -1, ws, size);
  return ws;
}

static inline char *webview_from_utf16(WCHAR *ws) {
  int n = WideCharToMultiByte(CP_UTF8, 0, ws, -1, NULL, 0, NULL, NULL);
  char *s = (char *)GlobalAlloc(GMEM_FIXED, n);
  if (s == NULL) {
    return NULL;
  }
  WideCharToMultiByte(CP_UTF8, 0, ws, -1, s, n, NULL, NULL);
  return s;
}

static int iid_eq(REFIID a, const IID *b) {
  return memcmp((const void *)iid_ref(a), (const void *)b, sizeof(GUID)) == 0;
}

static HRESULT STDMETHODCALLTYPE JS_QueryInterface(IDispatch *This,
                                                   REFIID riid,
                                                   LPVOID *ppvObj) {
  if (iid_eq(riid, &IID_IDispatch)) {
    *ppvObj = This;
    return S_OK;
  }
  *ppvObj = 0;
  return E_NOINTERFACE;
}
static ULONG STDMETHODCALLTYPE JS_AddRef(IDispatch *This) { return 1; }
static ULONG STDMETHODCALLTYPE JS_Release(IDispatch *This) { return 1; }
static HRESULT STDMETHODCALLTYPE JS_GetTypeInfoCount(IDispatch *This,
                                                     UINT *pctinfo) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE JS_GetTypeInfo(IDispatch *This,
                                                UINT iTInfo, LCID lcid,
                                                ITypeInfo **ppTInfo) {
  return S_OK;
}
#define WEBVIEW_JS_INVOKE_ID 0x1000
static HRESULT STDMETHODCALLTYPE JS_GetIDsOfNames(IDispatch *This,
                                                  REFIID riid,
                                                  LPOLESTR *rgszNames,
                                                  UINT cNames, LCID lcid,
                                                  DISPID *rgDispId) {
  if (cNames != 1) {
    return S_FALSE;
  }
  if (wcscmp(rgszNames[0], L"invoke") == 0) {
    rgDispId[0] = WEBVIEW_JS_INVOKE_ID;
    return S_OK;
  }
  return S_FALSE;
}

static HRESULT STDMETHODCALLTYPE
JS_Invoke(IDispatch *This, DISPID dispIdMember, REFIID riid, LCID lcid,
          WORD wFlags, DISPPARAMS *pDispParams, VARIANT *pVarResult,
          EXCEPINFO *pExcepInfo, UINT *puArgErr) {
  size_t offset = (size_t) & ((_IOleClientSiteEx *)NULL)->external;
  _IOleClientSiteEx *ex = (_IOleClientSiteEx *)((char *)(This)-offset);
  struct mshtml_webview *wv = (struct mshtml_webview *)GetWindowLongPtr(
      ex->inplace.frame.window, GWLP_USERDATA);
  if (pDispParams->cArgs == 1 && pDispParams->rgvarg[0].vt == VT_BSTR) {
    BSTR bstr = pDispParams->rgvarg[0].bstrVal;
    char *s = webview_from_utf16(bstr);
    if (s != NULL) {
      if (dispIdMember == WEBVIEW_JS_INVOKE_ID) {
        if (wv->external_invoke_cb != NULL) {
          wv->external_invoke_cb(wv, s);
        }
      } else {
        GlobalFree(s);
        return S_FALSE;
      }
      GlobalFree(s);
    }
  }
  return S_OK;
}

static IDispatchVtbl ExternalDispatchTable = {
    JS_QueryInterface, JS_AddRef,        JS_Release, JS_GetTypeInfoCount,
    JS_GetTypeInfo,    JS_GetIDsOfNames, JS_Invoke};

static BOOL EnableDpiAwareness() {
    // Use SetThreadDpiAwarenessContext if it's available (Windows 10).
    //
    // Use "SYSTEM_AWARE" because we haven't figure out how to make the browser
    // control properly handle DPI changes.
    HMODULE libUser32 = GetModuleHandleW(L"user32.dll");
    if (libUser32) {
        FnSetThreadDpiAwarenessContext SetThreadDpiAwarenessContext =
            (FnSetThreadDpiAwarenessContext) GetProcAddress(libUser32, "SetThreadDpiAwarenessContext");
        if(SetThreadDpiAwarenessContext != NULL) {
            if (SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE) != NULL) {
                return TRUE;
            }
        }
        // Otherwise fallback to SetProcessDPIAware. GCC can't handle the linking, so we use `GetProcAddress` too.
        FnSetProcessDPIAware SetProcessDPIAware =
          (FnSetProcessDPIAware) GetProcAddress(libUser32, "SetProcessDPIAware");
        if(SetProcessDPIAware != NULL) {
          return SetProcessDPIAware();
        }
    }
    return FALSE;
}

static ULONG STDMETHODCALLTYPE Site_AddRef(IOleClientSite *This) {
  return 1;
}
static ULONG STDMETHODCALLTYPE Site_Release(IOleClientSite *This) {
  return 1;
}
static HRESULT STDMETHODCALLTYPE Site_SaveObject(IOleClientSite *This) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE Site_GetMoniker(IOleClientSite *This,
                                                 DWORD dwAssign,
                                                 DWORD dwWhichMoniker,
                                                 IMoniker **ppmk) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE
Site_GetContainer(IOleClientSite *This, LPOLECONTAINER *ppContainer) {
  *ppContainer = 0;
  return E_NOINTERFACE;
}
static HRESULT STDMETHODCALLTYPE Site_ShowObject(IOleClientSite *This) {
  return NOERROR;
}
static HRESULT STDMETHODCALLTYPE Site_OnShowWindow(IOleClientSite *This,
                                                   BOOL fShow) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE
Site_RequestNewObjectLayout(IOleClientSite *This) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE Site_QueryInterface(IOleClientSite *This,
                                                     REFIID riid,
                                                     void **ppvObject) {
  if (iid_eq(riid, &IID_IUnknown) || iid_eq(riid, &IID_IOleClientSite)) {
    *ppvObject = &((_IOleClientSiteEx *)This)->client;
  } else if (iid_eq(riid, &IID_IOleInPlaceSite)) {
    *ppvObject = &((_IOleClientSiteEx *)This)->inplace;
  } else if (iid_eq(riid, &IID_IDocHostUIHandler)) {
    *ppvObject = &((_IOleClientSiteEx *)This)->ui;
  } else if (iid_eq(riid, &IID_IServiceProvider)) {
    *ppvObject = &((_IOleClientSiteEx *)This)->provider;
  } else {
    *ppvObject = 0;
    return (E_NOINTERFACE);
  }
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE InPlace_QueryInterface(
    IOleInPlaceSite *This, REFIID riid, LPVOID *ppvObj) {
  return (Site_QueryInterface(
      (IOleClientSite *)((char *)This - sizeof(IOleClientSite)), riid, ppvObj));
}
static ULONG STDMETHODCALLTYPE InPlace_AddRef(IOleInPlaceSite *This) {
  return 1;
}
static ULONG STDMETHODCALLTYPE InPlace_Release(IOleInPlaceSite *This) {
  return 1;
}
static HRESULT STDMETHODCALLTYPE InPlace_GetWindow(IOleInPlaceSite *This,
                                                   HWND *lphwnd) {
  *lphwnd = ((_IOleInPlaceSiteEx *)This)->frame.window;
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
InPlace_ContextSensitiveHelp(IOleInPlaceSite *This, BOOL fEnterMode) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE
InPlace_CanInPlaceActivate(IOleInPlaceSite *This) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
InPlace_OnInPlaceActivate(IOleInPlaceSite *This) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
InPlace_OnUIActivate(IOleInPlaceSite *This) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE InPlace_GetWindowContext(
    IOleInPlaceSite *This, LPOLEINPLACEFRAME *lplpFrame,
    LPOLEINPLACEUIWINDOW *lplpDoc, LPRECT lprcPosRect, LPRECT lprcClipRect,
    LPOLEINPLACEFRAMEINFO lpFrameInfo) {
  *lplpFrame = (LPOLEINPLACEFRAME) & ((_IOleInPlaceSiteEx *)This)->frame;
  *lplpDoc = 0;
  lpFrameInfo->fMDIApp = FALSE;
  lpFrameInfo->hwndFrame = ((_IOleInPlaceFrameEx *)*lplpFrame)->window;
  lpFrameInfo->haccel = 0;
  lpFrameInfo->cAccelEntries = 0;
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE InPlace_Scroll(IOleInPlaceSite *This,
                                                SIZE scrollExtent) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE
InPlace_OnUIDeactivate(IOleInPlaceSite *This, BOOL fUndoable) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
InPlace_OnInPlaceDeactivate(IOleInPlaceSite *This) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
InPlace_DiscardUndoState(IOleInPlaceSite *This) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE
InPlace_DeactivateAndUndo(IOleInPlaceSite *This) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE
InPlace_OnPosRectChange(IOleInPlaceSite *This, LPCRECT lprcPosRect) {
  IOleObject *browserObject;
  IOleInPlaceObject *inplace;
  browserObject = *((IOleObject **)((char *)This - sizeof(IOleObject *) -
                                    sizeof(IOleClientSite)));
  if (!browserObject->lpVtbl->QueryInterface(browserObject,
                                             iid_unref(&IID_IOleInPlaceObject),
                                             (void **)&inplace)) {
    inplace->lpVtbl->SetObjectRects(inplace, lprcPosRect, lprcPosRect);
    inplace->lpVtbl->Release(inplace);
  }
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE Frame_QueryInterface(
    IOleInPlaceFrame *This, REFIID riid, LPVOID *ppvObj) {
  return E_NOTIMPL;
}
static ULONG STDMETHODCALLTYPE Frame_AddRef(IOleInPlaceFrame *This) {
  return 1;
}
static ULONG STDMETHODCALLTYPE Frame_Release(IOleInPlaceFrame *This) {
  return 1;
}
static HRESULT STDMETHODCALLTYPE Frame_GetWindow(IOleInPlaceFrame *This,
                                                 HWND *lphwnd) {
  *lphwnd = ((_IOleInPlaceFrameEx *)This)->window;
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
Frame_ContextSensitiveHelp(IOleInPlaceFrame *This, BOOL fEnterMode) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE Frame_GetBorder(IOleInPlaceFrame *This,
                                                 LPRECT lprectBorder) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE Frame_RequestBorderSpace(
    IOleInPlaceFrame *This, LPCBORDERWIDTHS pborderwidths) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE Frame_SetBorderSpace(
    IOleInPlaceFrame *This, LPCBORDERWIDTHS pborderwidths) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE Frame_SetActiveObject(
    IOleInPlaceFrame *This, IOleInPlaceActiveObject *pActiveObject,
    LPCOLESTR pszObjName) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
Frame_InsertMenus(IOleInPlaceFrame *This, HMENU hmenuShared,
                  LPOLEMENUGROUPWIDTHS lpMenuWidths) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE Frame_SetMenu(IOleInPlaceFrame *This,
                                               HMENU hmenuShared,
                                               HOLEMENU holemenu,
                                               HWND hwndActiveObject) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE Frame_RemoveMenus(IOleInPlaceFrame *This,
                                                   HMENU hmenuShared) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE Frame_SetStatusText(IOleInPlaceFrame *This,
                                                     LPCOLESTR pszStatusText) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
Frame_EnableModeless(IOleInPlaceFrame *This, BOOL fEnable) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
Frame_TranslateAccelerator(IOleInPlaceFrame *This, LPMSG lpmsg, WORD wID) {
  return E_NOTIMPL;
}
static HRESULT STDMETHODCALLTYPE UI_QueryInterface(IDocHostUIHandler *This,
                                                   REFIID riid,
                                                   LPVOID *ppvObj) {
  return (Site_QueryInterface((IOleClientSite *)((char *)This -
                                                 sizeof(IOleClientSite) -
                                                 sizeof(_IOleInPlaceSiteEx)),
                              riid, ppvObj));
}
static ULONG STDMETHODCALLTYPE UI_AddRef(IDocHostUIHandler *This) {
  return 1;
}
static ULONG STDMETHODCALLTYPE UI_Release(IDocHostUIHandler *This) {
  return 1;
}
static HRESULT STDMETHODCALLTYPE UI_ShowContextMenu(
    IDocHostUIHandler *This, DWORD dwID, POINT *ppt,
    IUnknown *pcmdtReserved, IDispatch *pdispReserved) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
UI_GetHostInfo(IDocHostUIHandler *This, DOCHOSTUIINFO *pInfo) {
  pInfo->cbSize = sizeof(DOCHOSTUIINFO);
  pInfo->dwFlags = DOCHOSTUIFLAG_NO3DBORDER | DOCHOSTUIFLAG_DPI_AWARE;
  pInfo->dwDoubleClick = DOCHOSTUIDBLCLK_DEFAULT;
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE UI_ShowUI(
    IDocHostUIHandler *This, DWORD dwID,
    IOleInPlaceActiveObject *pActiveObject,
    IOleCommandTarget *pCommandTarget,
    IOleInPlaceFrame *pFrame, IOleInPlaceUIWindow *pDoc) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE UI_HideUI(IDocHostUIHandler *This) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE UI_UpdateUI(IDocHostUIHandler *This) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE UI_EnableModeless(IDocHostUIHandler *This,
                                                   BOOL fEnable) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
UI_OnDocWindowActivate(IDocHostUIHandler *This, BOOL fActivate) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
UI_OnFrameWindowActivate(IDocHostUIHandler *This, BOOL fActivate) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
UI_ResizeBorder(IDocHostUIHandler *This, LPCRECT prcBorder,
                IOleInPlaceUIWindow *pUIWindow, BOOL fRameWindow) {
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE
UI_TranslateAccelerator(IDocHostUIHandler *This, LPMSG lpMsg,
                        const GUID *pguidCmdGroup, DWORD nCmdID) {
  return S_FALSE;
}
static HRESULT STDMETHODCALLTYPE UI_GetOptionKeyPath(
    IDocHostUIHandler *This, LPOLESTR *pchKey, DWORD dw) {
  return S_FALSE;
}
static HRESULT STDMETHODCALLTYPE UI_GetDropTarget(
    IDocHostUIHandler *This, IDropTarget *pDropTarget,
    IDropTarget **ppDropTarget) {
  return S_FALSE;
}
static HRESULT STDMETHODCALLTYPE UI_GetExternal(
    IDocHostUIHandler *This, IDispatch **ppDispatch) {
  *ppDispatch = (IDispatch *)(This + 1);
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE UI_TranslateUrl(
    IDocHostUIHandler *This, DWORD dwTranslate, OLECHAR *pchURLIn,
    OLECHAR **ppchURLOut) {
  *ppchURLOut = 0;
  return S_FALSE;
}
static HRESULT STDMETHODCALLTYPE
UI_FilterDataObject(IDocHostUIHandler *This, IDataObject *pDO,
                    IDataObject **ppDORet) {
  *ppDORet = 0;
  return S_FALSE;
}

static const SAFEARRAYBOUND ArrayBound = {1, 0};

static IOleClientSiteVtbl MyIOleClientSiteTable = {
    Site_QueryInterface, Site_AddRef,       Site_Release,
    Site_SaveObject,     Site_GetMoniker,   Site_GetContainer,
    Site_ShowObject,     Site_OnShowWindow, Site_RequestNewObjectLayout};
static IOleInPlaceSiteVtbl MyIOleInPlaceSiteTable = {
    InPlace_QueryInterface,
    InPlace_AddRef,
    InPlace_Release,
    InPlace_GetWindow,
    InPlace_ContextSensitiveHelp,
    InPlace_CanInPlaceActivate,
    InPlace_OnInPlaceActivate,
    InPlace_OnUIActivate,
    InPlace_GetWindowContext,
    InPlace_Scroll,
    InPlace_OnUIDeactivate,
    InPlace_OnInPlaceDeactivate,
    InPlace_DiscardUndoState,
    InPlace_DeactivateAndUndo,
    InPlace_OnPosRectChange};

static IOleInPlaceFrameVtbl MyIOleInPlaceFrameTable = {
    Frame_QueryInterface,
    Frame_AddRef,
    Frame_Release,
    Frame_GetWindow,
    Frame_ContextSensitiveHelp,
    Frame_GetBorder,
    Frame_RequestBorderSpace,
    Frame_SetBorderSpace,
    Frame_SetActiveObject,
    Frame_InsertMenus,
    Frame_SetMenu,
    Frame_RemoveMenus,
    Frame_SetStatusText,
    Frame_EnableModeless,
    Frame_TranslateAccelerator};

static IDocHostUIHandlerVtbl MyIDocHostUIHandlerTable = {
    UI_QueryInterface,
    UI_AddRef,
    UI_Release,
    UI_ShowContextMenu,
    UI_GetHostInfo,
    UI_ShowUI,
    UI_HideUI,
    UI_UpdateUI,
    UI_EnableModeless,
    UI_OnDocWindowActivate,
    UI_OnFrameWindowActivate,
    UI_ResizeBorder,
    UI_TranslateAccelerator,
    UI_GetOptionKeyPath,
    UI_GetDropTarget,
    UI_GetExternal,
    UI_TranslateUrl,
    UI_FilterDataObject};



static HRESULT STDMETHODCALLTYPE IS_QueryInterface(IInternetSecurityManager *This, REFIID riid, void **ppvObject) {
  return E_NOTIMPL;
}
static ULONG STDMETHODCALLTYPE IS_AddRef(IInternetSecurityManager *This) { return 1; }
static ULONG STDMETHODCALLTYPE IS_Release(IInternetSecurityManager *This) { return 1; }
static HRESULT STDMETHODCALLTYPE IS_SetSecuritySite(IInternetSecurityManager *This, IInternetSecurityMgrSite *pSited) {
  return INET_E_DEFAULT_ACTION;
}
static HRESULT STDMETHODCALLTYPE IS_GetSecuritySite(IInternetSecurityManager *This, IInternetSecurityMgrSite **ppSite) {
  return INET_E_DEFAULT_ACTION;
}
static HRESULT STDMETHODCALLTYPE IS_MapUrlToZone(IInternetSecurityManager *This, LPCWSTR pwszUrl, DWORD *pdwZone, DWORD dwFlags) {
  *pdwZone = URLZONE_LOCAL_MACHINE;
  return S_OK;
}
static HRESULT STDMETHODCALLTYPE IS_GetSecurityId(IInternetSecurityManager *This, LPCWSTR pwszUrl, BYTE *pbSecurityId, DWORD *pcbSecurityId, DWORD_PTR dwReserved) {
  return INET_E_DEFAULT_ACTION;
}
static HRESULT STDMETHODCALLTYPE IS_ProcessUrlAction(IInternetSecurityManager *This, LPCWSTR pwszUrl, DWORD dwAction, BYTE *pPolicy,  DWORD cbPolicy, BYTE *pContext, DWORD cbContext, DWORD dwFlags, DWORD dwReserved) {
  return INET_E_DEFAULT_ACTION;
}
static HRESULT STDMETHODCALLTYPE IS_QueryCustomPolicy(IInternetSecurityManager *This, LPCWSTR pwszUrl, REFGUID guidKey, BYTE **ppPolicy, DWORD *pcbPolicy, BYTE *pContext, DWORD cbContext, DWORD dwReserved) {
  return INET_E_DEFAULT_ACTION;
}
static HRESULT STDMETHODCALLTYPE IS_SetZoneMapping(IInternetSecurityManager *This, DWORD dwZone, LPCWSTR lpszPattern, DWORD dwFlags) {
  return INET_E_DEFAULT_ACTION;
}
static HRESULT STDMETHODCALLTYPE IS_GetZoneMappings(IInternetSecurityManager *This, DWORD dwZone, IEnumString **ppenumString, DWORD dwFlags) {
  return INET_E_DEFAULT_ACTION;
}
static IInternetSecurityManagerVtbl MyInternetSecurityManagerTable = {IS_QueryInterface, IS_AddRef, IS_Release, IS_SetSecuritySite, IS_GetSecuritySite, IS_MapUrlToZone, IS_GetSecurityId, IS_ProcessUrlAction, IS_QueryCustomPolicy, IS_SetZoneMapping, IS_GetZoneMappings};

static HRESULT STDMETHODCALLTYPE SP_QueryInterface(IServiceProvider *This, REFIID riid, void **ppvObject) {
  return (Site_QueryInterface(
      (IOleClientSite *)((char *)This - sizeof(IOleClientSite) - sizeof(_IOleInPlaceSiteEx) - sizeof(_IDocHostUIHandlerEx) - sizeof(IDispatch)), riid, ppvObject));
}
static ULONG STDMETHODCALLTYPE SP_AddRef(IServiceProvider *This) { return 1; }
static ULONG STDMETHODCALLTYPE SP_Release(IServiceProvider *This) { return 1; }
static HRESULT STDMETHODCALLTYPE SP_QueryService(IServiceProvider *This, REFGUID siid, REFIID riid, void **ppvObject) {
  if (iid_eq(siid, &IID_IInternetSecurityManager) && iid_eq(riid, &IID_IInternetSecurityManager)) {
    *ppvObject = &((_IServiceProviderEx *)This)->mgr;
  } else {
    *ppvObject = 0;
    return (E_NOINTERFACE);
  }
  return S_OK;
}
static IServiceProviderVtbl MyServiceProviderTable = {SP_QueryInterface, SP_AddRef, SP_Release, SP_QueryService};

static void UnEmbedBrowserObject(webview_t w) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  if (wv->browser != NULL) {
    (*wv->browser)->lpVtbl->Close(*wv->browser, OLECLOSE_NOSAVE);
    (*wv->browser)->lpVtbl->Release(*wv->browser);
    GlobalFree(wv->browser);
    wv->browser = NULL;
  }
}


static int EmbedBrowserObject(webview_t w) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  RECT rect;
  IWebBrowser2 *webBrowser2 = NULL;
  LPCLASSFACTORY pClassFactory = NULL;
  _IOleClientSiteEx *_iOleClientSiteEx = NULL;
  IOleObject **browser = (IOleObject **)GlobalAlloc(
      GMEM_FIXED, sizeof(IOleObject *) + sizeof(_IOleClientSiteEx));
  if (browser == NULL) {
    goto error;
  }
  wv->browser = browser;

  _iOleClientSiteEx = (_IOleClientSiteEx *)(browser + 1);
  _iOleClientSiteEx->client.lpVtbl = &MyIOleClientSiteTable;
  _iOleClientSiteEx->inplace.inplace.lpVtbl = &MyIOleInPlaceSiteTable;
  _iOleClientSiteEx->inplace.frame.frame.lpVtbl = &MyIOleInPlaceFrameTable;
  _iOleClientSiteEx->inplace.frame.window = wv->hwnd;
  _iOleClientSiteEx->ui.ui.lpVtbl = &MyIDocHostUIHandlerTable;
  _iOleClientSiteEx->external.lpVtbl = &ExternalDispatchTable;
  _iOleClientSiteEx->provider.provider.lpVtbl = &MyServiceProviderTable;
  _iOleClientSiteEx->provider.mgr.mgr.lpVtbl = &MyInternetSecurityManagerTable;

  if (CoGetClassObject(iid_unref(&CLSID_WebBrowser),
                       CLSCTX_INPROC_SERVER | CLSCTX_INPROC_HANDLER, NULL,
                       iid_unref(&IID_IClassFactory),
                       (void **)&pClassFactory) != S_OK) {
    goto error;
  }

  if (pClassFactory == NULL) {
    goto error;
  }

  if (pClassFactory->lpVtbl->CreateInstance(pClassFactory, 0,
                                            iid_unref(&IID_IOleObject),
                                            (void **)browser) != S_OK) {
    goto error;
  }
  pClassFactory->lpVtbl->Release(pClassFactory);
  if ((*browser)->lpVtbl->SetClientSite(
          *browser, (IOleClientSite *)_iOleClientSiteEx) != S_OK) {
    goto error;
  }
  (*browser)->lpVtbl->SetHostNames(*browser, L"My Host Name", 0);

  if (OleSetContainedObject((struct IUnknown *)(*browser), TRUE) != S_OK) {
    goto error;
  }
  GetClientRect(wv->hwnd, &rect);
  if ((*browser)->lpVtbl->DoVerb((*browser), OLEIVERB_SHOW, NULL,
                                 (IOleClientSite *)_iOleClientSiteEx, -1,
                                 wv->hwnd, &rect) != S_OK) {
    goto error;
  }
  if ((*browser)->lpVtbl->QueryInterface((*browser),
                                         iid_unref(&IID_IWebBrowser2),
                                         (void **)&webBrowser2) != S_OK) {
    goto error;
  }

  webBrowser2->lpVtbl->put_Left(webBrowser2, 0);
  webBrowser2->lpVtbl->put_Top(webBrowser2, 0);
  webBrowser2->lpVtbl->put_Width(webBrowser2, rect.right);
  webBrowser2->lpVtbl->put_Height(webBrowser2, rect.bottom);
  webBrowser2->lpVtbl->Release(webBrowser2);

  return 0;
error:
  UnEmbedBrowserObject(w);
  if (pClassFactory != NULL) {
    pClassFactory->lpVtbl->Release(pClassFactory);
  }
  if (browser != NULL) {
    GlobalFree(browser);
  }
  return -1;
}

#define WEBVIEW_DATA_URL_PREFIX "data:text/html,"
static int DisplayHTMLPage(struct mshtml_webview *wv) {
  IWebBrowser2 *webBrowser2;
  VARIANT myURL;
  LPDISPATCH lpDispatch;
  IHTMLDocument2 *htmlDoc2;
  BSTR bstr;
  IOleObject *browserObject;
  SAFEARRAY *sfArray;
  VARIANT *pVar;
  browserObject = *wv->browser;
  int isDataURL = 0;
  const char *webview_url = wv->url == NULL ? "" : wv->url;
  if (!browserObject->lpVtbl->QueryInterface(
          browserObject, iid_unref(&IID_IWebBrowser2), (void **)&webBrowser2)) {
    LPCSTR webPageName;
    isDataURL = (strncmp(webview_url, WEBVIEW_DATA_URL_PREFIX,
                         strlen(WEBVIEW_DATA_URL_PREFIX)) == 0);
    if (isDataURL) {
      webPageName = "about:blank";
    } else {
      webPageName = (LPCSTR)webview_url;
    }
    VariantInit(&myURL);
    myURL.vt = VT_BSTR;
    myURL.bstrVal = webview_to_bstr(webPageName);
    if (!myURL.bstrVal) {
    badalloc:
      webBrowser2->lpVtbl->Release(webBrowser2);
      return (-6);
    }
    webBrowser2->lpVtbl->Navigate2(webBrowser2, &myURL, 0, 0, 0, 0);
    VariantClear(&myURL);
    if (!isDataURL) {
      return 0;
    }

    char *url = (char *)calloc(1, strlen(webview_url) + 1);
    char *q = url;
    for (const char *p = webview_url + strlen(WEBVIEW_DATA_URL_PREFIX); *q = *p;
         p++, q++) {
      if (*q == '%' && *(p + 1) && *(p + 2)) {
        *q = hex2char(p + 1);
        p = p + 2;
      }
    }

    if (webBrowser2->lpVtbl->get_Document(webBrowser2, &lpDispatch) == S_OK) {
      if (lpDispatch->lpVtbl->QueryInterface(lpDispatch,
                                             iid_unref(&IID_IHTMLDocument2),
                                             (void **)&htmlDoc2) == S_OK) {
        if ((sfArray = SafeArrayCreate(VT_VARIANT, 1,
                                       (SAFEARRAYBOUND *)&ArrayBound))) {
          if (!SafeArrayAccessData(sfArray, (void **)&pVar)) {
            pVar->vt = VT_BSTR;
            bstr = webview_to_bstr(url);
            if ((pVar->bstrVal = bstr)) {
              htmlDoc2->lpVtbl->write(htmlDoc2, sfArray);
              htmlDoc2->lpVtbl->close(htmlDoc2);
            }
          }
          SafeArrayDestroy(sfArray);
        }
      release:
        free(url);
        htmlDoc2->lpVtbl->Release(htmlDoc2);
      }
      lpDispatch->lpVtbl->Release(lpDispatch);
    }
    webBrowser2->lpVtbl->Release(webBrowser2);
    return (0);
  }
  return (-5);
}

LRESULT CALLBACK wndproc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam) {
  struct mshtml_webview *wv = (struct mshtml_webview *)GetWindowLongPtr(hwnd, GWLP_USERDATA);
  switch (uMsg) {
  case WM_CREATE:
    wv = (struct mshtml_webview *)((CREATESTRUCT *)lParam)->lpCreateParams;
    wv->hwnd = hwnd;
    return EmbedBrowserObject(wv);
  case WM_DESTROY:
    UnEmbedBrowserObject(wv);
    PostQuitMessage(0);
    return TRUE;
  case WM_SIZE: {
    IWebBrowser2 *webBrowser2;
    IOleObject *browser = *wv->browser;
    if (browser->lpVtbl->QueryInterface(browser, iid_unref(&IID_IWebBrowser2),
                                        (void **)&webBrowser2) == S_OK) {
      RECT rect;
      GetClientRect(hwnd, &rect);
      webBrowser2->lpVtbl->put_Width(webBrowser2, rect.right);
      webBrowser2->lpVtbl->put_Height(webBrowser2, rect.bottom);
    }
    return TRUE;
  }
  case WM_WEBVIEW_DISPATCH: {
    webview_dispatch_fn f = (webview_dispatch_fn)wParam;
    void *arg = (void *)lParam;
    (*f)(wv, arg);
    return TRUE;
  }
  case WM_GETMINMAXINFO:
  {
    if (wv) {
      LPMINMAXINFO lpMMI = (LPMINMAXINFO)lParam;
      lpMMI->ptMinTrackSize.x = wv->min_width;
      lpMMI->ptMinTrackSize.y = wv->min_height;
    }
      
    break;
  }
  }
  return DefWindowProc(hwnd, uMsg, wParam, lParam);
}

WEBVIEW_API int webview_loop(webview_t w, int blocking) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  MSG msg;
  if (blocking) {
    if (GetMessage(&msg, 0, 0, 0) < 0) return 0;
  } else {
    if (PeekMessage(&msg, 0, 0, 0, PM_REMOVE) == 0) return 0;
  }
  switch (msg.message) {
  case WM_QUIT:
    return -1;
  case WM_COMMAND:
  case WM_KEYDOWN:
  case WM_KEYUP: {
    HRESULT r = S_OK;
    IWebBrowser2 *webBrowser2;
    IOleObject *browser = *wv->browser;
    if (browser->lpVtbl->QueryInterface(browser, iid_unref(&IID_IWebBrowser2),
                                        (void **)&webBrowser2) == S_OK) {
      IOleInPlaceActiveObject *pIOIPAO;
      if (browser->lpVtbl->QueryInterface(
              browser, iid_unref(&IID_IOleInPlaceActiveObject),
              (void **)&pIOIPAO) == S_OK) {
        r = pIOIPAO->lpVtbl->TranslateAccelerator(pIOIPAO, &msg);
        pIOIPAO->lpVtbl->Release(pIOIPAO);
      }
      webBrowser2->lpVtbl->Release(webBrowser2);
    }
    if (r != S_FALSE) {
      break;
    }
  }
  default:
    TranslateMessage(&msg);
    DispatchMessage(&msg);
  }
  return 0;
}

WEBVIEW_API int webview_eval(webview_t w, const char *js) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  IWebBrowser2 *webBrowser2;
  IHTMLDocument2 *htmlDoc2;
  IDispatch *docDispatch;
  IDispatch *scriptDispatch;
  if ((*wv->browser)
          ->lpVtbl->QueryInterface((*wv->browser),
                                   iid_unref(&IID_IWebBrowser2),
                                   (void **)&webBrowser2) != S_OK) {
    return -1;
  }

  if (webBrowser2->lpVtbl->get_Document(webBrowser2, &docDispatch) != S_OK) {
    return -1;
  }
  if (docDispatch->lpVtbl->QueryInterface(docDispatch,
                                          iid_unref(&IID_IHTMLDocument2),
                                          (void **)&htmlDoc2) != S_OK) {
    return -1;
  }
  if (htmlDoc2->lpVtbl->get_Script(htmlDoc2, &scriptDispatch) != S_OK) {
    return -1;
  }
  DISPID dispid;
  LPOLESTR evalStr = L"eval";
  if (scriptDispatch->lpVtbl->GetIDsOfNames(
          scriptDispatch, iid_unref(&IID_NULL), &evalStr, 1,
          LOCALE_SYSTEM_DEFAULT, &dispid) != S_OK) {
    return -1;
  }

  DISPPARAMS params;
  VARIANT arg;
  VARIANT result;
  EXCEPINFO excepInfo;
  UINT nArgErr = (UINT)-1;
  params.cArgs = 1;
  params.cNamedArgs = 0;
  params.rgvarg = &arg;
  arg.vt = VT_BSTR;
  static const char *prologue = "(function(){";
  static const char *epilogue = ";})();";
  int n = strlen(prologue) + strlen(epilogue) + strlen(js) + 1;
  char *eval = (char *)malloc(n);
  if (eval == NULL) {
    return -1;
  }
  snprintf(eval, n, "%s%s%s", prologue, js, epilogue);
  arg.bstrVal = webview_to_bstr(eval);
  free(eval);
  if (arg.bstrVal == NULL) {
    return -1;
  }
  if (scriptDispatch->lpVtbl->Invoke(
          scriptDispatch, dispid, iid_unref(&IID_NULL), 0, DISPATCH_METHOD,
          &params, &result, &excepInfo, &nArgErr) != S_OK) {
    SysFreeString(arg.bstrVal);
    return -1;
  }
  SysFreeString(arg.bstrVal);
  scriptDispatch->lpVtbl->Release(scriptDispatch);
  htmlDoc2->lpVtbl->Release(htmlDoc2);
  docDispatch->lpVtbl->Release(docDispatch);
  return 0;
}

WEBVIEW_API void webview_dispatch(webview_t w, webview_dispatch_fn fn,
                                  void *arg) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  PostMessageW(wv->hwnd, WM_WEBVIEW_DISPATCH, (WPARAM)fn, (LPARAM)arg);
}

WEBVIEW_API void webview_set_title(webview_t w, const char *title) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  BSTR window_title = webview_to_bstr(title);
  SetWindowText(wv->hwnd, window_title);
  SysFreeString(window_title);
}

WEBVIEW_API void webview_set_fullscreen(webview_t w, int fullscreen) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  if (wv->is_fullscreen == !!fullscreen) {
    return;
  }
  if (wv->is_fullscreen == 0) {
    wv->saved_style = GetWindowLong(wv->hwnd, GWL_STYLE);
    wv->saved_ex_style = GetWindowLong(wv->hwnd, GWL_EXSTYLE);
    GetWindowRect(wv->hwnd, &wv->saved_rect);
  }
  wv->is_fullscreen = !!fullscreen;
  if (fullscreen) {
    MONITORINFO monitor_info;
    SetWindowLong(wv->hwnd, GWL_STYLE,
                  wv->saved_style & ~(WS_CAPTION | WS_THICKFRAME));
    SetWindowLong(wv->hwnd, GWL_EXSTYLE,
                  wv->saved_ex_style &
                      ~(WS_EX_DLGMODALFRAME | WS_EX_WINDOWEDGE |
                        WS_EX_CLIENTEDGE | WS_EX_STATICEDGE));
    monitor_info.cbSize = sizeof(monitor_info);
    GetMonitorInfo(MonitorFromWindow(wv->hwnd, MONITOR_DEFAULTTONEAREST),
                   &monitor_info);
    RECT r;
    r.left = monitor_info.rcMonitor.left;
    r.top = monitor_info.rcMonitor.top;
    r.right = monitor_info.rcMonitor.right;
    r.bottom = monitor_info.rcMonitor.bottom;
    SetWindowPos(wv->hwnd, NULL, r.left, r.top, r.right - r.left,
                 r.bottom - r.top,
                 SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
  } else {
    SetWindowLong(wv->hwnd, GWL_STYLE, wv->saved_style);
    SetWindowLong(wv->hwnd, GWL_EXSTYLE, wv->saved_ex_style);
    SetWindowPos(wv->hwnd, NULL, wv->saved_rect.left,
                 wv->saved_rect.top,
                 wv->saved_rect.right - wv->saved_rect.left,
                 wv->saved_rect.bottom - wv->saved_rect.top,
                 SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
  }
}

WEBVIEW_API void webview_set_maximized(webview_t w, int maximize) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  BOOL is_maximized = IsZoomed(wv->hwnd);
  if (is_maximized == maximize) {
    return;
  }
  if (!is_maximized) {
    GetWindowRect(wv->hwnd, &wv->saved_rect);
  }
  if (maximize) {
    RECT r;

    SystemParametersInfoW(SPI_GETWORKAREA, 0, &r, 0);
    
    ShowWindow(wv->hwnd, SW_MAXIMIZE);
    SetWindowPos(wv->hwnd, NULL, r.left, r.top, r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
  } else {
    ShowWindow(wv->hwnd, SW_RESTORE);
    SetWindowPos(wv->hwnd, NULL, wv->saved_rect.left,
                wv->saved_rect.top,
                wv->saved_rect.right - wv->saved_rect.left,
                wv->saved_rect.bottom - wv->saved_rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
  }
}

WEBVIEW_API void webview_set_minimized(webview_t w, int minimize){
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  BOOL is_minimized = IsIconic(wv->hwnd);
  if (is_minimized == minimize)
      return;
  if (minimize)
      ShowWindow(wv->hwnd, SW_MINIMIZE);
  else
      ShowWindow(wv->hwnd, SW_RESTORE);
}

WEBVIEW_API void webview_set_visible(webview_t w, int visible) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;

  ShowWindow(wv->hwnd, visible ? SW_SHOW : SW_HIDE);
}

WEBVIEW_API void webview_set_color(webview_t w, uint8_t r, uint8_t g,
                                   uint8_t b, uint8_t a) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  HBRUSH brush = CreateSolidBrush(RGB(r, g, b));
  SetClassLongPtr(wv->hwnd, GCLP_HBRBACKGROUND, (LONG_PTR)brush);
}

WEBVIEW_API void webview_exit(webview_t w) {
  struct mshtml_webview* wv = (struct mshtml_webview*)w;
  DestroyWindow(wv->hwnd);
  OleUninitialize();
}

WEBVIEW_API void webview_print_log(const char *s) { OutputDebugString(s); }
