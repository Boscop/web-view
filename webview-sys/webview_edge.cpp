#include "webview.h"

#include <atomic>
#include <cstring>
#include <functional>
#include <future>
#include <map>
#include <string>
#include <vector>

#define WIN32_LEAN_AND_MEAN
#include <objbase.h>
#include <windows.h>
#include <wingdi.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Web.UI.Interop.h>

#pragma comment(lib, "windowsapp.lib")
#pragma comment(lib, "user32.lib")
#pragma comment(lib, "gdi32.lib")
#pragma comment(lib, "ole32.lib")

// Free result with SysFreeString.
static inline BSTR webview_to_bstr(const char *s) {
  DWORD size = MultiByteToWideChar(CP_UTF8, 0, s, -1, 0, 0);
  BSTR bs = SysAllocStringLen(0, size);
  if (bs == NULL) {
    return NULL;
  }
  MultiByteToWideChar(CP_UTF8, 0, s, -1, bs, size);
  return bs;
}

namespace webview {
using dispatch_fn_t = std::function<void()>;
using msg_cb_t = std::function<void(const char* msg)>;

inline std::string url_decode(const char *s)
{
    std::string decoded;
    size_t length = strlen(s);
    for (unsigned int i = 0; i < length; i++) {
        if (s[i] == '%') {
            int n;
            sscanf(s + i + 1, "%2x", &n);
            decoded.push_back(static_cast<char>(n));
            i = i + 2;
        } else if (s[i] == '+') {
            decoded.push_back(' ');
        } else {
            decoded.push_back(s[i]);
        }
    }
    return decoded;
}

inline std::string html_from_uri(const char *s)
{
    const char *const prefix = "data:text/html,";
    const size_t prefix_length = strlen(prefix);
    if (!strncmp(s, prefix, prefix_length)) {
      return url_decode(s + prefix_length);
    }
    return "";
}

LRESULT CALLBACK WebviewWndProc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp);
class browser_window {
public:
    browser_window(msg_cb_t cb, const char* title, int width, int height, bool resizable)
        : m_cb(cb)
    {
        HINSTANCE hInstance = GetModuleHandle(nullptr);

        WNDCLASSEX wc;
        ZeroMemory(&wc, sizeof(WNDCLASSEX));
        wc.cbSize = sizeof(WNDCLASSEX);
        wc.hInstance = hInstance;
        wc.lpfnWndProc = WebviewWndProc;
        wc.lpszClassName = L"webview";
        RegisterClassEx(&wc);

        DWORD style = WS_OVERLAPPEDWINDOW;
        if (!resizable) {
            style = WS_OVERLAPPED | WS_CAPTION | WS_MINIMIZEBOX | WS_SYSMENU;
        }

        RECT clientRect;
        RECT rect;
        rect.left = 0;
        rect.top = 0;
        rect.right = width;
        rect.bottom = height;
        AdjustWindowRect(&rect, WS_OVERLAPPEDWINDOW, 0);

        GetClientRect(GetDesktopWindow(), &clientRect);
        int left = (clientRect.right / 2) - ((rect.right - rect.left) / 2);
        int top = (clientRect.bottom / 2) - ((rect.bottom - rect.top) / 2);
        rect.right = rect.right - rect.left + left;
        rect.left = left;
        rect.bottom = rect.bottom - rect.top + top;
        rect.top = top;

        BSTR window_title = webview_to_bstr(title);
        m_window = CreateWindowEx(0, L"webview", window_title, style, rect.left, rect.top,
                     rect.right - rect.left, rect.bottom - rect.top,
                     HWND_DESKTOP, NULL, hInstance, (void *)this);
        SysFreeString(window_title);

        SetWindowLongPtr(m_window, GWLP_USERDATA, (LONG_PTR)this);

        ShowWindow(m_window, SW_SHOW);
        UpdateWindow(m_window);
        SetFocus(m_window);
    }

    void run()
    {
        while (this->loop(true) == 0) {

        }
    }

    int loop(int blocking)
    {
        MSG msg;

        if (blocking) {
            if (GetMessage(&msg, nullptr, 0, 0) < 0) return 0;
        } else {
            if (PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE) == 0) return 0;
        }

        if (msg.hwnd) {
            TranslateMessage(&msg);
            DispatchMessage(&msg);
            return 0;
        }
        if (msg.message == WM_APP) {
            auto f = (dispatch_fn_t*)(msg.lParam);
            (*f)();
            delete f;
        } else if (msg.message == WM_QUIT) {
            return -1;
        }

        return 0;
    }

    void dispatch(dispatch_fn_t f)
    {
        PostThreadMessage(m_main_thread, WM_APP, 0, (LPARAM) new dispatch_fn_t(f));
    }

    void set_title(const char* title)
    {
        BSTR window_title = webview_to_bstr(title);
        SetWindowText(m_window, window_title);
        SysFreeString(window_title);
    }

    void set_size(int width, int height)
    {
        RECT r;
        r.left = 50;
        r.top = 50;
        r.right = width;
        r.bottom = height;
        AdjustWindowRect(&r, WS_OVERLAPPEDWINDOW, 0);
        SetWindowPos(m_window, NULL, r.left, r.top, r.right - r.left, r.bottom - r.top,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    }

    void set_fullscreen(bool fullscreen)
    {
        if (this->is_fullscreen == fullscreen) {
            return;
        }
        if (!this->is_fullscreen) {
            this->saved_style = GetWindowLong(this->m_window, GWL_STYLE);
            this->saved_ex_style = GetWindowLong(this->m_window, GWL_EXSTYLE);
            GetWindowRect(this->m_window, &this->saved_rect);
        }
        this->is_fullscreen = !!fullscreen;
        if (fullscreen) {
            MONITORINFO monitor_info;
            SetWindowLong(this->m_window, GWL_STYLE,
                        this->saved_style & ~(WS_CAPTION | WS_THICKFRAME));
            SetWindowLong(this->m_window, GWL_EXSTYLE,
                        this->saved_ex_style &
                            ~(WS_EX_DLGMODALFRAME | WS_EX_WINDOWEDGE |
                                WS_EX_CLIENTEDGE | WS_EX_STATICEDGE));
            monitor_info.cbSize = sizeof(monitor_info);
            GetMonitorInfo(MonitorFromWindow(this->m_window, MONITOR_DEFAULTTONEAREST),
                        &monitor_info);
            RECT r;
            r.left = monitor_info.rcMonitor.left;
            r.top = monitor_info.rcMonitor.top;
            r.right = monitor_info.rcMonitor.right;
            r.bottom = monitor_info.rcMonitor.bottom;
            SetWindowPos(this->m_window, NULL, r.left, r.top, r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        } else {
            SetWindowLong(this->m_window, GWL_STYLE, this->saved_style);
            SetWindowLong(this->m_window, GWL_EXSTYLE, this->saved_ex_style);
            SetWindowPos(this->m_window, NULL, this->saved_rect.left,
                        this->saved_rect.top,
                        this->saved_rect.right - this->saved_rect.left,
                        this->saved_rect.bottom - this->saved_rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        }
    }

    void set_color(uint8_t r, uint8_t g, uint8_t b, uint8_t a)
    {
        HBRUSH brush = CreateSolidBrush(RGB(r, g, b));
        SetClassLongPtr(this->m_window, GCLP_HBRBACKGROUND, (LONG_PTR)brush);
    }

    // protected:
    virtual void resize() {}
    HWND m_window;
    DWORD m_main_thread = GetCurrentThreadId();
    msg_cb_t m_cb;

    bool is_fullscreen = false;
    DWORD saved_style = 0;
    DWORD saved_ex_style = 0;
    RECT saved_rect;
};

LRESULT CALLBACK WebviewWndProc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp)
{
    auto w = (browser_window*)GetWindowLongPtr(hwnd, GWLP_USERDATA);
    switch (msg) {
    case WM_SIZE:
        w->resize();
        break;
    case WM_CLOSE:
        DestroyWindow(hwnd);
        break;
    case WM_DESTROY:
        w->exit();
        break;
    default:
        return DefWindowProc(hwnd, msg, wp, lp);
    }
    return 0;
}

using namespace winrt;
using namespace Windows::Foundation;
using namespace Windows::Web::UI;
using namespace Windows::Web::UI::Interop;

class webview : public browser_window {
public:
    webview(webview_external_invoke_cb_t invoke_cb, const char* title, int width, int height, bool resizable, bool debug)
        : browser_window(std::bind(&webview::on_message, this, std::placeholders::_1), title, width, height, resizable)
        , invoke_cb(invoke_cb)
    {
        init_apartment(winrt::apartment_type::single_threaded);
        WebViewControlProcessOptions options;
        options.PrivateNetworkClientServerCapability(WebViewControlProcessCapabilityState::Enabled);
        m_process = WebViewControlProcess(options);
        auto op = m_process.CreateWebViewControlAsync(
            reinterpret_cast<int64_t>(m_window), Rect());
        if (op.Status() != AsyncStatus::Completed) {
            handle h(CreateEvent(nullptr, false, false, nullptr));
            op.Completed([h = h.get()](auto, auto) { SetEvent(h); });
            HANDLE hs[] = { h.get() };
            DWORD i;
            CoWaitForMultipleHandles(COWAIT_DISPATCH_WINDOW_MESSAGES
                    | COWAIT_DISPATCH_CALLS | COWAIT_INPUTAVAILABLE,
                INFINITE, 1, hs, &i);
        }
        m_webview = op.GetResults();
        m_webview.Settings().IsScriptNotifyAllowed(true);
        m_webview.IsVisible(true);
        m_webview.ScriptNotify([=](auto const& sender, auto const& args) {
            std::string s = winrt::to_string(args.Value());
            m_cb(s.c_str());
        });
        m_webview.NavigationStarting([=](auto const& sender, auto const& args) {
            m_webview.AddInitializeScript(winrt::to_hstring(init_js));
        });
        init("window.external.invoke = s => window.external.notify(s)");
        resize();
    }

    void navigate(const char* url)
    {
        std::string html = html_from_uri(url);
        if (html != "") {
            m_webview.NavigateToString(winrt::to_hstring(html.c_str()));
        } else {
            Uri uri(winrt::to_hstring(url));
            m_webview.Navigate(uri);
        }
    }
    void init(const char* js)
    {
      init_js.append("(function(){")
             .append(js)
             .append("})();");
    }
    void eval(const char* js)
    {
        m_webview.InvokeScriptAsync(
            L"eval", single_threaded_vector<hstring>({ winrt::to_hstring(js) }));
    }

    void exit() {
        m_webview.Close();
    }

    void* window() { return (void*)m_window; }

    void* get_user_data() { return this->user_data; }
    void set_user_data(void* user_data) { this->user_data = user_data; }
private:
    void on_message(const char* msg)
    {
        this->invoke_cb(this, msg);
    }

    void resize()
    {
        RECT r;
        GetClientRect(m_window, &r);
        Rect bounds(r.left, r.top, r.right - r.left, r.bottom - r.top);
        m_webview.Bounds(bounds);
    }
    WebViewControlProcess m_process;
    WebViewControl m_webview = nullptr;
    std::string init_js = "";

    void* user_data = nullptr;
    webview_external_invoke_cb_t invoke_cb = nullptr;
};

} // namespace webview

// Free result with GlobalFree.
static inline char *webview_from_utf16(WCHAR *ws) {
  int n = WideCharToMultiByte(CP_UTF8, 0, ws, -1, NULL, 0, NULL, NULL);
  char *s = (char *)GlobalAlloc(GMEM_FIXED, n);
  if (s == NULL) {
    return NULL;
  }
  WideCharToMultiByte(CP_UTF8, 0, ws, -1, s, n, NULL, NULL);
  return s;
}

#include <shobjidl.h>

#ifdef __cplusplus
#define iid_ref(x) &(x)
#define iid_unref(x) *(x)
#else
#define iid_ref(x) (x)
#define iid_unref(x) (x)
#endif

/* These are missing parts from MinGW */
#ifndef __IFileDialog_INTERFACE_DEFINED__
#define __IFileDialog_INTERFACE_DEFINED__
enum _FILEOPENDIALOGOPTIONS {
  FOS_OVERWRITEPROMPT = 0x2,
  FOS_STRICTFILETYPES = 0x4,
  FOS_NOCHANGEDIR = 0x8,
  FOS_PICKFOLDERS = 0x20,
  FOS_FORCEFILESYSTEM = 0x40,
  FOS_ALLNONSTORAGEITEMS = 0x80,
  FOS_NOVALIDATE = 0x100,
  FOS_ALLOWMULTISELECT = 0x200,
  FOS_PATHMUSTEXIST = 0x800,
  FOS_FILEMUSTEXIST = 0x1000,
  FOS_CREATEPROMPT = 0x2000,
  FOS_SHAREAWARE = 0x4000,
  FOS_NOREADONLYRETURN = 0x8000,
  FOS_NOTESTFILECREATE = 0x10000,
  FOS_HIDEMRUPLACES = 0x20000,
  FOS_HIDEPINNEDPLACES = 0x40000,
  FOS_NODEREFERENCELINKS = 0x100000,
  FOS_DONTADDTORECENT = 0x2000000,
  FOS_FORCESHOWHIDDEN = 0x10000000,
  FOS_DEFAULTNOMINIMODE = 0x20000000,
  FOS_FORCEPREVIEWPANEON = 0x40000000
};
typedef DWORD FILEOPENDIALOGOPTIONS;
typedef enum FDAP { FDAP_BOTTOM = 0, FDAP_TOP = 1 } FDAP;
DEFINE_GUID(IID_IFileDialog, 0x42f85136, 0xdb7e, 0x439c, 0x85, 0xf1, 0xe4, 0x07,
            0x5d, 0x13, 0x5f, 0xc8);
typedef struct IFileDialogVtbl {
  BEGIN_INTERFACE
  HRESULT(STDMETHODCALLTYPE *QueryInterface)
  (IFileDialog *This, REFIID riid, void **ppvObject);
  ULONG(STDMETHODCALLTYPE *AddRef)(IFileDialog *This);
  ULONG(STDMETHODCALLTYPE *Release)(IFileDialog *This);
  HRESULT(STDMETHODCALLTYPE *Show)(IFileDialog *This, HWND hwndOwner);
  HRESULT(STDMETHODCALLTYPE *SetFileTypes)
  (IFileDialog *This, UINT cFileTypes, const COMDLG_FILTERSPEC *rgFilterSpec);
  HRESULT(STDMETHODCALLTYPE *SetFileTypeIndex)
  (IFileDialog *This, UINT iFileType);
  HRESULT(STDMETHODCALLTYPE *GetFileTypeIndex)
  (IFileDialog *This, UINT *piFileType);
  HRESULT(STDMETHODCALLTYPE *Advise)
  (IFileDialog *This, IFileDialogEvents *pfde, DWORD *pdwCookie);
  HRESULT(STDMETHODCALLTYPE *Unadvise)(IFileDialog *This, DWORD dwCookie);
  HRESULT(STDMETHODCALLTYPE *SetOptions)
  (IFileDialog *This, FILEOPENDIALOGOPTIONS fos);
  HRESULT(STDMETHODCALLTYPE *GetOptions)
  (IFileDialog *This, FILEOPENDIALOGOPTIONS *pfos);
  HRESULT(STDMETHODCALLTYPE *SetDefaultFolder)
  (IFileDialog *This, IShellItem *psi);
  HRESULT(STDMETHODCALLTYPE *SetFolder)(IFileDialog *This, IShellItem *psi);
  HRESULT(STDMETHODCALLTYPE *GetFolder)(IFileDialog *This, IShellItem **ppsi);
  HRESULT(STDMETHODCALLTYPE *GetCurrentSelection)
  (IFileDialog *This, IShellItem **ppsi);
  HRESULT(STDMETHODCALLTYPE *SetFileName)(IFileDialog *This, LPCWSTR pszName);
  HRESULT(STDMETHODCALLTYPE *GetFileName)(IFileDialog *This, LPWSTR *pszName);
  HRESULT(STDMETHODCALLTYPE *SetTitle)(IFileDialog *This, LPCWSTR pszTitle);
  HRESULT(STDMETHODCALLTYPE *SetOkButtonLabel)
  (IFileDialog *This, LPCWSTR pszText);
  HRESULT(STDMETHODCALLTYPE *SetFileNameLabel)
  (IFileDialog *This, LPCWSTR pszLabel);
  HRESULT(STDMETHODCALLTYPE *GetResult)(IFileDialog *This, IShellItem **ppsi);
  HRESULT(STDMETHODCALLTYPE *AddPlace)
  (IFileDialog *This, IShellItem *psi, FDAP fdap);
  HRESULT(STDMETHODCALLTYPE *SetDefaultExtension)
  (IFileDialog *This, LPCWSTR pszDefaultExtension);
  HRESULT(STDMETHODCALLTYPE *Close)(IFileDialog *This, HRESULT hr);
  HRESULT(STDMETHODCALLTYPE *SetClientGuid)(IFileDialog *This, REFGUID guid);
  HRESULT(STDMETHODCALLTYPE *ClearClientData)(IFileDialog *This);
  HRESULT(STDMETHODCALLTYPE *SetFilter)
  (IFileDialog *This, IShellItemFilter *pFilter);
  END_INTERFACE
} IFileDialogVtbl;
interface IFileDialog {
  CONST_VTBL IFileDialogVtbl *lpVtbl;
};
DEFINE_GUID(IID_IFileOpenDialog, 0xd57c7288, 0xd4ad, 0x4768, 0xbe, 0x02, 0x9d,
            0x96, 0x95, 0x32, 0xd9, 0x60);
DEFINE_GUID(IID_IFileSaveDialog, 0x84bccd23, 0x5fde, 0x4cdb, 0xae, 0xa4, 0xaf,
            0x64, 0xb8, 0x3d, 0x78, 0xab);
#endif


// HERE

WEBVIEW_API void webview_run(webview_t w)
{
    static_cast<webview::webview*>(w)->run();
}

WEBVIEW_API int webview_loop(webview_t w, int blocking)
{
    return static_cast<webview::webview*>(w)->loop(blocking);
}

WEBVIEW_API int webview_eval(webview_t w, const char *js)
{
    static_cast<webview::webview*>(w)->eval(js);
    return 0;
}

WEBVIEW_API void webview_set_title(webview_t w, const char *title)
{
    static_cast<webview::webview*>(w)->set_title(title);
}

WEBVIEW_API void webview_set_fullscreen(webview_t w, int fullscreen)
{
    static_cast<webview::webview*>(w)->set_fullscreen(fullscreen);
}

WEBVIEW_API void webview_set_color(webview_t w, uint8_t r, uint8_t g,
                                   uint8_t b, uint8_t a)
{
    static_cast<webview::webview*>(w)->set_color(r, g, b, a);
}

WEBVIEW_API void webview_dialog(webview_t w,
                    enum webview_dialog_type dlgtype, int flags,
                    const char *title, const char *arg,
                    char *result, size_t resultsz) {
  HWND hwnd = static_cast<webview::webview*>(w)->m_window;
  if (dlgtype == WEBVIEW_DIALOG_TYPE_OPEN ||
      dlgtype == WEBVIEW_DIALOG_TYPE_SAVE) {
    IFileDialog *dlg = NULL;
    IShellItem *res = NULL;
    WCHAR *ws = NULL;
    char *s = NULL;
    FILEOPENDIALOGOPTIONS opts = 0, add_opts = 0;
    if (dlgtype == WEBVIEW_DIALOG_TYPE_OPEN) {
      if (CoCreateInstance(
              iid_unref(&CLSID_FileOpenDialog), NULL, CLSCTX_INPROC_SERVER,
              iid_unref(&IID_IFileOpenDialog), (void **)&dlg) != S_OK) {
        goto error_dlg;
      }
      if (flags & WEBVIEW_DIALOG_FLAG_DIRECTORY) {
        add_opts |= FOS_PICKFOLDERS;
      }
      add_opts |= FOS_NOCHANGEDIR | FOS_ALLNONSTORAGEITEMS | FOS_NOVALIDATE |
                  FOS_PATHMUSTEXIST | FOS_FILEMUSTEXIST | FOS_SHAREAWARE |
                  FOS_NOTESTFILECREATE | FOS_NODEREFERENCELINKS |
                  FOS_FORCESHOWHIDDEN | FOS_DEFAULTNOMINIMODE;
    } else {
      if (CoCreateInstance(
              iid_unref(&CLSID_FileSaveDialog), NULL, CLSCTX_INPROC_SERVER,
              iid_unref(&IID_IFileSaveDialog), (void **)&dlg) != S_OK) {
        goto error_dlg;
      }
      add_opts |= FOS_OVERWRITEPROMPT | FOS_NOCHANGEDIR |
                  FOS_ALLNONSTORAGEITEMS | FOS_NOVALIDATE | FOS_SHAREAWARE |
                  FOS_NOTESTFILECREATE | FOS_NODEREFERENCELINKS |
                  FOS_FORCESHOWHIDDEN | FOS_DEFAULTNOMINIMODE;
    }
    if (dlg->GetOptions(&opts) != S_OK) {
      goto error_dlg;
    }
    opts &= ~FOS_NOREADONLYRETURN;
    opts |= add_opts;
    if (dlg->SetOptions(opts) != S_OK) {
      goto error_dlg;
    }
    if (dlg->Show(hwnd) != S_OK) {
      goto error_dlg;
    }
    if (dlg->GetResult(&res) != S_OK) {
      goto error_dlg;
    }
    if (res->GetDisplayName(SIGDN_FILESYSPATH, &ws) != S_OK) {
      goto error_result;
    }
    s = webview_from_utf16(ws);
    CoTaskMemFree(ws);
    if (!s) goto error_result;
    strncpy(result, s, resultsz);
    GlobalFree(s);
    result[resultsz - 1] = '\0';
  error_result:
    res->Release();
  error_dlg:
    dlg->Release();
    return;
  } else if (dlgtype == WEBVIEW_DIALOG_TYPE_ALERT) {
#if 0
    /* MinGW often doesn't contain TaskDialog, we'll use MessageBox for now */
    WCHAR *wtitle = webview_to_utf16(title);
    WCHAR *warg = webview_to_utf16(arg);
    TaskDialog(hwnd, NULL, NULL, wtitle, warg, 0, NULL, NULL);
    GlobalFree(warg);
    GlobalFree(wtitle);
#else
    UINT type = MB_OK;
    switch (flags & WEBVIEW_DIALOG_FLAG_ALERT_MASK) {
    case WEBVIEW_DIALOG_FLAG_INFO:
      type |= MB_ICONINFORMATION;
      break;
    case WEBVIEW_DIALOG_FLAG_WARNING:
      type |= MB_ICONWARNING;
      break;
    case WEBVIEW_DIALOG_FLAG_ERROR:
      type |= MB_ICONERROR;
      break;
    }
    BSTR box_title = webview_to_bstr(title);
    BSTR box_text = webview_to_bstr(arg);
    MessageBox(hwnd, box_text, box_title, type);
    SysFreeString(box_title);
    SysFreeString(box_text);
#endif
  }
}

WEBVIEW_API void webview_dispatch(webview_t w, webview_dispatch_fn fn,
                                  void *arg)
{
    static_cast<webview::webview*>(w)->dispatch([=]() { fn(w, arg); });
}

WEBVIEW_API void webview_exit(webview_t w)
{
    webview::webview* wv = static_cast<webview::webview*>(w);
    DestroyWindow(wv->m_window);
}

WEBVIEW_API void webview_debug(const char *format, ...)
{
    // TODO
}

WEBVIEW_API void webview_print_log(const char *s)
{
    // TODO
}

WEBVIEW_API void* webview_get_user_data(webview_t w)
{
    return static_cast<webview::webview*>(w)->get_user_data();
}

WEBVIEW_API webview_t webview_new(const char* title, const char* url, int width, int height, int resizable, int debug, webview_external_invoke_cb_t external_invoke_cb, void* userdata)
{
    auto w = new webview::webview(external_invoke_cb, title, width, height, resizable, debug);
    w->set_user_data(userdata);
    w->navigate(url);
	return w;
}

WEBVIEW_API void webview_free(webview_t w)
{
    delete static_cast<webview::webview*>(w);
}

WEBVIEW_API void webview_destroy(webview_t w)
{
    delete static_cast<webview::webview*>(w);
}
