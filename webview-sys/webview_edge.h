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

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void* webview_t;

typedef void (*webview_external_invoke_cb_t)(webview_t w, const char* arg);

// Create a new webview instance
WEBVIEW_API webview_t webview_create(webview_external_invoke_cb_t invoke_cb, int width, int height, int resizable, int debug);

// Destroy a webview
WEBVIEW_API void webview_destroy(webview_t w);

// Run the main loop
WEBVIEW_API void webview_run(webview_t w);
WEBVIEW_API int webview_loop(webview_t w, int blocking);
WEBVIEW_API int webview_eval(webview_t w, const char* js);
// Inject css into webview's page
WEBVIEW_API int webview_inject_css(webview_t w, const char *css);
WEBVIEW_API void webview_set_title(webview_t w, const char* title);
// Enable or disable window fullscreen
WEBVIEW_API void webview_set_fullscreen(webview_t w, int fullscreen);
// Set rgba color of the window's title bar
WEBVIEW_API void webview_set_color(webview_t w, uint8_t r, uint8_t g, uint8_t b, uint8_t a);
WEBVIEW_API void webview_dialog(webview_t w,
                                enum webview_dialog_type dlgtype, int flags,
                                const char *title, const char *arg,
                                char *result, size_t resultsz);
// Post a function to be executed on the main thread
WEBVIEW_API void webview_dispatch(webview_t w, void (*fn)(webview_t w, void* arg), void* arg);
// Stop the main loop
WEBVIEW_API void webview_terminate(webview_t w);
WEBVIEW_API void webview_navigate(webview_t w, const char* url);
WEBVIEW_API void* webview_get_userdata(webview_t w);
WEBVIEW_API void webview_set_userdata(webview_t w, void* user_data);

#ifdef __cplusplus
}
#endif

enum webview_dialog_type {
  WEBVIEW_DIALOG_TYPE_OPEN = 0,
  WEBVIEW_DIALOG_TYPE_SAVE = 1,
  WEBVIEW_DIALOG_TYPE_ALERT = 2
};

#define WEBVIEW_DIALOG_FLAG_FILE (0 << 0)
#define WEBVIEW_DIALOG_FLAG_DIRECTORY (1 << 0)

#define WEBVIEW_DIALOG_FLAG_INFO (1 << 1)
#define WEBVIEW_DIALOG_FLAG_WARNING (2 << 1)
#define WEBVIEW_DIALOG_FLAG_ERROR (3 << 1)
#define WEBVIEW_DIALOG_FLAG_ALERT_MASK (3 << 1)

#ifndef WEBVIEW_HEADER



#endif /* WEBVIEW_HEADER */

#endif /* WEBVIEW_H */
