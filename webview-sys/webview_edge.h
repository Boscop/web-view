/*
 * MIT License
 *
 * Copyright (c) 2017 Serge Zaitsev
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */
#ifndef WEBVIEW_H
#define WEBVIEW_H

#ifndef WEBVIEW_API
#define WEBVIEW_API extern
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef void* webview_t;

typedef void (*webview_external_invoke_cb_t)(webview_t w, const char* arg);

// Create a new webview instance
WEBVIEW_API webview_t webview_create(int debug, webview_external_invoke_cb_t invoke_cb, void* wnd);

// Destroy a webview
WEBVIEW_API void webview_destroy(webview_t w);

// Run the main loop
WEBVIEW_API void webview_run(webview_t w);

// Stop the main loop
WEBVIEW_API void webview_terminate(webview_t w);

// Post a function to be executed on the main thread
WEBVIEW_API void webview_dispatch(
    webview_t w, void (*fn)(webview_t w, void* arg), void* arg);

WEBVIEW_API void* webview_get_window(webview_t w);

WEBVIEW_API void webview_set_title(webview_t w, const char* title);

WEBVIEW_API void webview_set_bounds(
    webview_t w, int x, int y, int width, int height, int flags);
WEBVIEW_API void webview_get_bounds(
    webview_t w, int* x, int* y, int* width, int* height, int* flags);

WEBVIEW_API void webview_navigate(webview_t w, const char* url);
WEBVIEW_API void webview_init(webview_t w, const char* js);
WEBVIEW_API int webview_eval(webview_t w, const char* js);

WEBVIEW_API int webview_loop(webview_t w, int blocking);

WEBVIEW_API void* webview_get_userdata(webview_t w);
WEBVIEW_API void webview_set_userdata(webview_t w, void* user_data);

#ifdef __cplusplus
}
#endif

#ifndef WEBVIEW_HEADER

#include <atomic>
#include <cstring>
#include <functional>
#include <future>
#include <map>
#include <string>
#include <vector>

//
// ====================================================================
//
// This implementation uses Win32 API to create a native window. It can
// use either MSHTML or EdgeHTML backend as a browser engine.
//
// ====================================================================
//

#define WIN32_LEAN_AND_MEAN
#include <objbase.h>
#include <windows.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Web.UI.Interop.h>

#pragma comment(lib, "windowsapp")
#pragma comment(lib, "user32.lib")

namespace webview {
using dispatch_fn_t = std::function<void()>;
using msg_cb_t = std::function<void(const char* msg)>;

inline std::string url_encode(std::string s)
{
    std::string encoded;
    for (unsigned int i = 0; i < s.length(); i++) {
        auto c = s[i];
        if (isalnum(c) || c == '-' || c == '_' || c == '.' || c == '~') {
            encoded = encoded + c;
        } else {
            char hex[4];
            snprintf(hex, sizeof(hex), "%%%02x", c);
            encoded = encoded + hex;
        }
    }
    return encoded;
}

inline std::string url_decode(std::string s)
{
    std::string decoded;
    for (unsigned int i = 0; i < s.length(); i++) {
        if (s[i] == '%') {
            int n;
            sscanf(s.substr(i + 1, 2).c_str(), "%x", &n);
            decoded = decoded + static_cast<char>(n);
            i = i + 2;
        } else if (s[i] == '+') {
            decoded = decoded + ' ';
        } else {
            decoded = decoded + s[i];
        }
    }
    return decoded;
}

inline std::string html_from_uri(std::string s)
{
    if (s.substr(0, 15) == "data:text/html,") {
        return url_decode(s.substr(15));
    }
    return "";
}

inline int json_parse_c(const char* s, size_t sz, const char* key, size_t keysz,
    const char** value, size_t* valuesz)
{
    enum {
        JSON_STATE_VALUE,
        JSON_STATE_LITERAL,
        JSON_STATE_STRING,
        JSON_STATE_ESCAPE,
        JSON_STATE_UTF8
    } state
        = JSON_STATE_VALUE;
    const char* k = NULL;
    int index = 1;
    int depth = 0;
    int utf8_bytes = 0;

    if (key == NULL) {
        index = keysz;
        keysz = 0;
    }

    *value = NULL;
    *valuesz = 0;

    for (; sz > 0; s++, sz--) {
        enum {
            JSON_ACTION_NONE,
            JSON_ACTION_START,
            JSON_ACTION_END,
            JSON_ACTION_START_STRUCT,
            JSON_ACTION_END_STRUCT
        } action
            = JSON_ACTION_NONE;
        unsigned char c = *s;
        switch (state) {
        case JSON_STATE_VALUE:
            if (c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == ',' || c == ':') {
                continue;
            } else if (c == '"') {
                action = JSON_ACTION_START;
                state = JSON_STATE_STRING;
            } else if (c == '{' || c == '[') {
                action = JSON_ACTION_START_STRUCT;
            } else if (c == '}' || c == ']') {
                action = JSON_ACTION_END_STRUCT;
            } else if (c == 't' || c == 'f' || c == 'n' || c == '-'
                || (c >= '0' && c <= '9')) {
                action = JSON_ACTION_START;
                state = JSON_STATE_LITERAL;
            } else {
                return -1;
            }
            break;
        case JSON_STATE_LITERAL:
            if (c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == ',' || c == ']'
                || c == '}' || c == ':') {
                state = JSON_STATE_VALUE;
                s--;
                sz++;
                action = JSON_ACTION_END;
            } else if (c < 32 || c > 126) {
                return -1;
            } // fallthrough
        case JSON_STATE_STRING:
            if (c < 32 || (c > 126 && c < 192)) {
                return -1;
            } else if (c == '"') {
                action = JSON_ACTION_END;
                state = JSON_STATE_VALUE;
            } else if (c == '\\') {
                state = JSON_STATE_ESCAPE;
            } else if (c >= 192 && c < 224) {
                utf8_bytes = 1;
                state = JSON_STATE_UTF8;
            } else if (c >= 224 && c < 240) {
                utf8_bytes = 2;
                state = JSON_STATE_UTF8;
            } else if (c >= 240 && c < 247) {
                utf8_bytes = 3;
                state = JSON_STATE_UTF8;
            } else if (c >= 128 && c < 192) {
                return -1;
            }
            break;
        case JSON_STATE_ESCAPE:
            if (c == '"' || c == '\\' || c == '/' || c == 'b' || c == 'f' || c == 'n'
                || c == 'r' || c == 't' || c == 'u') {
                state = JSON_STATE_STRING;
            } else {
                return -1;
            }
            break;
        case JSON_STATE_UTF8:
            if (c < 128 || c > 191) {
                return -1;
            }
            utf8_bytes--;
            if (utf8_bytes == 0) {
                state = JSON_STATE_STRING;
            }
            break;
        default:
            return -1;
        }

        if (action == JSON_ACTION_END_STRUCT) {
            depth--;
        }

        if (depth == 1) {
            if (action == JSON_ACTION_START || action == JSON_ACTION_START_STRUCT) {
                if (index == 0) {
                    *value = s;
                } else if (keysz > 0 && index == 1) {
                    k = s;
                } else {
                    index--;
                }
            } else if (action == JSON_ACTION_END || action == JSON_ACTION_END_STRUCT) {
                if (*value != NULL && index == 0) {
                    *valuesz = (size_t)(s + 1 - *value);
                    return 0;
                } else if (keysz > 0 && k != NULL) {
                    if (keysz == (size_t)(s - k - 1) && memcmp(key, k + 1, keysz) == 0) {
                        index = 0;
                    } else {
                        index = 2;
                    }
                    k = NULL;
                }
            }
        }

        if (action == JSON_ACTION_START_STRUCT) {
            depth++;
        }
    }
    return -1;
}

inline std::string json_escape(std::string s)
{
    // TODO: implement
    return '"' + s + '"';
}

inline int json_unescape(const char* s, size_t n, char* out)
{
    int r = 0;
    if (*s++ != '"') {
        return -1;
    }
    while (n > 2) {
        char c = *s;
        if (c == '\\') {
            s++;
            n--;
            switch (*s) {
            case 'b':
                c = '\b';
                break;
            case 'f':
                c = '\f';
                break;
            case 'n':
                c = '\n';
                break;
            case 'r':
                c = '\r';
                break;
            case 't':
                c = '\t';
                break;
            case '\\':
                c = '\\';
                break;
            case '/':
                c = '/';
                break;
            case '\"':
                c = '\"';
                break;
            default: // TODO: support unicode decoding
                return -1;
            }
        }
        if (out != NULL) {
            *out++ = c;
        }
        s++;
        n--;
        r++;
    }
    if (*s != '"') {
        return -1;
    }
    if (out != NULL) {
        *out = '\0';
    }
    return r;
}

inline std::string json_parse(std::string s, std::string key, int index)
{
    const char* value;
    size_t value_sz;
    if (key == "") {
        json_parse_c(s.c_str(), s.length(), nullptr, index, &value, &value_sz);
    } else {
        json_parse_c(s.c_str(), s.length(), key.c_str(), key.length(), &value, &value_sz);
    }
    if (value != nullptr) {
        if (value[0] != '"') {
            return std::string(value, value_sz);
        }
        int n = json_unescape(value, value_sz, nullptr);
        if (n > 0) {
            char* decoded = new char[n];
            json_unescape(value, value_sz, decoded);
            auto result = std::string(decoded, n);
            delete[] decoded;
            return result;
        }
    }
    return "";
}

LRESULT CALLBACK WebviewWndProc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp);
class browser_window {
public:
    browser_window(msg_cb_t cb, void* window)
        : m_cb(cb)
    {
        if (window == nullptr) {
            WNDCLASSEX wc;
            ZeroMemory(&wc, sizeof(WNDCLASSEX));
            wc.cbSize = sizeof(WNDCLASSEX);
            wc.hInstance = GetModuleHandle(nullptr);
            wc.lpszClassName = "webview";
            wc.lpfnWndProc = WebviewWndProc;
            RegisterClassEx(&wc);
            m_window = CreateWindow("webview", "", WS_OVERLAPPEDWINDOW, CW_USEDEFAULT,
                CW_USEDEFAULT, 640, 480, nullptr, nullptr, GetModuleHandle(nullptr),
                nullptr);
            SetWindowLongPtr(m_window, GWLP_USERDATA, (LONG_PTR)this);
        } else {
            m_window = *(static_cast<HWND*>(window));
        }

        ShowWindow(m_window, SW_SHOW);
        UpdateWindow(m_window);
        SetFocus(m_window);
    }

    void run()
    {
        // MSG msg;
        // BOOL res;
        // while ((res = GetMessage(&msg, nullptr, 0, 0)) != -1) {
        //     if (msg.hwnd) {
        //         TranslateMessage(&msg);
        //         DispatchMessage(&msg);
        //         continue;
        //     }
        //     if (msg.message == WM_APP) {
        //         auto f = (dispatch_fn_t*)(msg.lParam);
        //         (*f)();
        //         delete f;
        //     } else if (msg.message == WM_QUIT) {
        //         return;
        //     }
        // }
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

    void terminate() { PostQuitMessage(0); }
    void dispatch(dispatch_fn_t f)
    {
        PostThreadMessage(m_main_thread, WM_APP, 0, (LPARAM) new dispatch_fn_t(f));
    }

    void set_title(const char* title) { SetWindowText(m_window, title); }

    void set_size(int width, int height, bool resizable)
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

    // protected:
    virtual void resize() {}
    HWND m_window;
    DWORD m_main_thread = GetCurrentThreadId();
    msg_cb_t m_cb;
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
        w->terminate();
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
    webview(webview_external_invoke_cb_t invoke_cb, bool debug, void* window)
        : browser_window(std::bind(&webview::on_message, this, std::placeholders::_1), window)
        , invoke_cb(invoke_cb)
    {
        init_apartment(winrt::apartment_type::single_threaded);
        m_process = WebViewControlProcess();
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
    void init(const char* js) { init_js = init_js + "(function(){" + js + "})();"; }
    void eval(const char* js)
    {
        m_webview.InvokeScriptAsync(
            L"eval", single_threaded_vector<hstring>({ winrt::to_hstring(js) }));
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
    webview_external_invoke_cb_t invoke_cb;
};

} // namespace webview

WEBVIEW_API webview_t webview_create(webview_external_invoke_cb_t invoke_cb, int debug, void* wnd)
{
    return new webview::webview(invoke_cb, debug, wnd);
}

WEBVIEW_API void webview_destroy(webview_t w)
{
    delete static_cast<webview::webview*>(w);
}

WEBVIEW_API void webview_run(webview_t w) { static_cast<webview::webview*>(w)->run(); }

WEBVIEW_API void webview_terminate(webview_t w)
{
    static_cast<webview::webview*>(w)->terminate();
}

WEBVIEW_API void webview_dispatch(
    webview_t w, void (*fn)(webview_t w, void* arg), void* arg)
{
    static_cast<webview::webview*>(w)->dispatch([=]() { fn(w, arg); });
}

WEBVIEW_API void* webview_get_window(webview_t w)
{
    return static_cast<webview::webview*>(w)->window();
}

WEBVIEW_API void webview_set_title(webview_t w, const char* title)
{
    static_cast<webview::webview*>(w)->set_title(title);
}

WEBVIEW_API void webview_set_bounds(
    webview_t w, int x, int y, int width, int height, int flags)
{
    // TODO: x, y, flags
    static_cast<webview::webview*>(w)->set_size(width, height, true);
}

WEBVIEW_API void webview_get_bounds(
    webview_t w, int* x, int* y, int* width, int* height, int* flags)
{
    // TODO
}

WEBVIEW_API void webview_navigate(webview_t w, const char* url)
{
    static_cast<webview::webview*>(w)->navigate(url);
}

WEBVIEW_API void webview_init(webview_t w, const char* js)
{
    static_cast<webview::webview*>(w)->init(js);
}

WEBVIEW_API int webview_eval(webview_t w, const char* js)
{
    static_cast<webview::webview*>(w)->eval(js);
    return 0;
}

WEBVIEW_API int webview_loop(webview_t w, int blocking) {
	return static_cast<webview::webview*>(w)->loop(blocking);
}

WEBVIEW_API void* webview_get_userdata(webview_t w) {
    return static_cast<webview::webview*>(w)->get_user_data();
}

WEBVIEW_API void webview_set_userdata(webview_t w, void* user_data)
{
    static_cast<webview::webview*>(w)->set_user_data(user_data);
}

#endif /* WEBVIEW_HEADER */

#endif /* WEBVIEW_H */
