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
#include <winrt/Windows.Foundation.Collections.h>

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
            decoded.push_back(hex2char(s + i + 1));
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
private:
    bool EnableDpiAwareness() {
        auto lib_user32 = GetModuleHandleW(L"user32.dll");
        if(lib_user32) {
            auto fn_set_thread_dpi_awareness_context =
                reinterpret_cast<decltype(&SetThreadDpiAwarenessContext)>(
                GetProcAddress(lib_user32, "SetThreadDpiAwarenessContext")
            );
            if (
                fn_set_thread_dpi_awareness_context
                && fn_set_thread_dpi_awareness_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
            ) {
                return true;
            }
        }
        // Don't worry when SetThreadDpiAwarenessContext is not available. If
        // it's not available, we are not on a recent windows 10, and we don't
        // have edge...
        return false;
    }
    int MyGetDpiForWindow(HWND hWnd) {
        auto lib_user32 = GetModuleHandleW(L"user32.dll");
        if (lib_user32) {
            auto fn_get_dpi_for_window = reinterpret_cast<decltype(&GetDpiForWindow)>(
                GetProcAddress(lib_user32, "GetDpiForWindow")
            );
            if (fn_get_dpi_for_window) {
                return fn_get_dpi_for_window(hWnd);
            }
        }
        // Again, don't worry when GetDpiForWindow is not available. If we are
        // not on Windows 10, we don't have edge...
        return 96;
    }
public:
    browser_window(msg_cb_t cb, const char* title, int width, int height, bool resizable, bool frameless, bool visible, int min_width, int min_height, int hide_instead_of_close)
        : m_cb(cb), m_min_width(min_width), m_min_height(min_height), m_hide_instead_of_close(hide_instead_of_close)
    {
        HINSTANCE hInstance = GetModuleHandle(nullptr);

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
        wc.lpfnWndProc = WebviewWndProc;
        wc.lpszClassName = L"webview";
        wc.hIcon = winresIcon;
        RegisterClassEx(&wc);

        EnableDpiAwareness();

        DWORD style = WS_OVERLAPPEDWINDOW;
        if (!resizable) {
            style &= ~(WS_SIZEBOX | WS_MAXIMIZEBOX);
        }

        if (frameless) {
            style &= ~(WS_SYSMENU | WS_CAPTION | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
        }

        // Create window first, because we need the window to get DPI for the window.
        BSTR window_title = webview_to_bstr(title);
        m_window = CreateWindowEx(0, L"webview", window_title, style, 0, 0, 0, 0,
                     HWND_DESKTOP, NULL, hInstance, (void *)this);
        SysFreeString(window_title);

        // Have to call this before SetWindowPos or it will crash!
        SetWindowLongPtr(m_window, GWLP_USERDATA, (LONG_PTR)this);
        if (frameless)
        {
            SetWindowLongPtr(m_window, GWL_STYLE, style);
        }
        this->saved_style = style;

        UINT dpi = MyGetDpiForWindow(m_window);
        if (dpi == 0) {
            dpi = 96;
        }

        RECT clientRect;
        RECT rect;
        rect.left = 0;
        rect.top = 0;
        rect.right = MulDiv(width, dpi, 96);
        rect.bottom = MulDiv(height, dpi, 96);
        AdjustWindowRect(&rect, WS_OVERLAPPEDWINDOW, 0);
        GetClientRect(GetDesktopWindow(), &clientRect);
        int left = (clientRect.right / 2) - ((rect.right - rect.left) / 2);
        int top = (clientRect.bottom / 2) - ((rect.bottom - rect.top) / 2);
        rect.right = rect.right - rect.left + left;
        rect.left = left;
        rect.bottom = rect.bottom - rect.top + top;
        rect.top = top;

        // Set position, size and show window *atomically*.
        SetWindowPos(m_window, HWND_TOP, rect.left, rect.top,
            rect.right - rect.left, rect.bottom - rect.top, visible ? SWP_SHOWWINDOW : SWP_HIDEWINDOW);

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
    void exit() { PostQuitMessage(0); }
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

    void set_minimized(bool minimize)
    {
        bool is_minimized = IsIconic(this->m_window);
        if (is_minimized == minimize) {
            set_maximized(true);
            return;
        }
        if (minimize)
            ShowWindow(this->m_window, SW_MINIMIZE);
        else
            ShowWindow(this->m_window, SW_RESTORE);
    }

    void set_maximized(bool maximize)
    {
        bool is_maximized = IsZoomed(this->m_window);
        if (is_maximized == maximize) {
            return;
        }
        if (!is_maximized) {
            GetWindowRect(this->m_window, &this->saved_rect);
        }
        if (maximize) {
            RECT r;

            SystemParametersInfoW(SPI_GETWORKAREA, 0, &r, 0);
            ShowWindow(this->m_window, SW_MAXIMIZE);
            SetWindowPos(this->m_window, NULL, 0, 0, r.right - r.left,
                        r.bottom - r.top,
                        SWP_SHOWWINDOW);
        } else {
            ShowWindow(this->m_window, SW_RESTORE);
            SetWindowPos(this->m_window, NULL, this->saved_rect.left,
                        this->saved_rect.top,
                        this->saved_rect.right - this->saved_rect.left,
                        this->saved_rect.bottom - this->saved_rect.top,
                        SWP_SHOWWINDOW);
        }
    }

    void set_visible(bool visible)
    {
        ShowWindow(this->m_window, visible ? SW_SHOW : SW_HIDE);
    }

    void set_color(uint8_t r, uint8_t g, uint8_t b, uint8_t a)
    {

        HBRUSH brush = CreateSolidBrush(RGB(r, g, b));
        SetClassLongPtr(this->m_window, GCLP_HBRBACKGROUND, (LONG_PTR)brush);
    }

    int get_min_width() {
        return this->m_min_width;
    }

    int get_min_height() {
        return this->m_min_height;
    }

    bool get_hide_instead_of_close() {
        return this->m_hide_instead_of_close;
    }

    // protected:
    virtual void resize() {}
    HWND m_window;
    DWORD m_main_thread = GetCurrentThreadId();
    msg_cb_t m_cb;

    bool is_fullscreen = false;
    bool is_maximized = false;
    DWORD saved_style = 0;
    DWORD saved_ex_style = 0;
    RECT saved_rect;

    int m_min_width, m_min_height;
    bool m_hide_instead_of_close = false;
};

LRESULT CALLBACK WebviewWndProc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp)
{
    auto w = (browser_window*)GetWindowLongPtr(hwnd, GWLP_USERDATA);
    switch (msg) {
    case WM_SIZE:
        w->resize();
        break;
    case WM_DPICHANGED: {
        auto rect = reinterpret_cast<LPRECT>(lp);
        auto x = rect->left;
        auto y = rect->top;
        auto w = rect->right - x;
        auto h = rect->bottom - y;
        SetWindowPos(hwnd, nullptr, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
        break;
    }
    case WM_CLOSE:
        if (w->get_hide_instead_of_close()) {
            w->set_visible(false);
        } else {
            DestroyWindow(hwnd);
        }
        
        break;
    case WM_DESTROY:
        w->exit();
        break;
    case WM_GETMINMAXINFO: {
        if (w) {
            LPMINMAXINFO lpMMI = reinterpret_cast<LPMINMAXINFO>(lp);
            lpMMI->ptMinTrackSize.x = w->get_min_width();
            lpMMI->ptMinTrackSize.y = w->get_min_height();
        }

        break;
    }

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
    webview(webview_external_invoke_cb_t invoke_cb, const char* title, int width, int height, bool resizable, bool debug, bool frameless, bool visible, int min_width, int min_height, int hide_instead_of_close)
        : browser_window(std::bind(&webview::on_message, this, std::placeholders::_1), title, width, height, resizable, frameless, visible, min_width, min_height, hide_instead_of_close)
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
    void set_html(const char* html)
    {
        m_webview.NavigateToString(winrt::to_hstring(html));
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

WEBVIEW_API void webview_set_maximized(webview_t w, int maximize)
{
    static_cast<webview::webview*>(w)->set_maximized(maximize);
}

WEBVIEW_API void webview_set_minimized(webview_t w, int minimize)
{
    static_cast<webview::webview*>(w)->set_minimized(minimize);
}

WEBVIEW_API void webview_set_visible(webview_t w, int visible)
{
    static_cast<webview::webview*>(w)->set_visible(visible);
}

WEBVIEW_API void webview_set_color(webview_t w, uint8_t r, uint8_t g,
                                   uint8_t b, uint8_t a)
{
    static_cast<webview::webview*>(w)->set_color(r, g, b, a);
}

WEBVIEW_API void webview_set_zoom_level(webview_t w, const double percentage) {
    // Ignored on EdgeHTML
}

WEBVIEW_API void webview_set_html(webview_t w, const char *html) {
    static_cast<webview::webview*>(w)->set_html(html);
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

WEBVIEW_API void* webview_get_window_handle(webview_t w)
{
    return static_cast<webview::webview*>(w)->window();
}

WEBVIEW_API webview_t webview_new(
    const char* title, const char* url, int width, int height, int resizable, int debug,
    int frameless, int visible, int min_width, int min_height, int hide_instead_of_close, webview_external_invoke_cb_t external_invoke_cb, void* userdata)
{
    auto w = new webview::webview(external_invoke_cb, title, width, height, resizable, debug, frameless, visible, min_width, min_height, hide_instead_of_close);
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
