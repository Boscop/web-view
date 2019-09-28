#include "webview_edge.h"

#ifdef __cplusplus
extern "C" {
#endif

void wrapper_webview_free(webview_t w) {
	webview_destroy(w);
}

webview_t wrapper_webview_new(const char* title, const char* url, int width, int height, int resizable, int debug, webview_external_invoke_cb_t external_invoke_cb, void* userdata) {
	webview_t w = webview_create(external_invoke_cb, title, width, height, resizable, debug);
	webview_set_userdata(w, userdata);
	webview_set_title(w, title);
	webview_navigate(w, url);
	return w;
}

void* wrapper_webview_get_userdata(webview_t w) {
	return webview_get_userdata(w);
}

void webview_exit(webview_t w) {
	webview_terminate(w);
}

static int webview_js_encode(const char *s, char *esc, size_t n) {
  int r = 1; /* At least one byte for trailing zero */
  for (; *s; s++) {
    const unsigned char c = *s;
    if (c >= 0x20 && c < 0x80 && strchr("<>\\'\"", c) == NULL) {
      if (n > 0) {
        *esc++ = c;
        n--;
      }
      r++;
    } else {
      if (n > 0) {
        snprintf(esc, n, "\\x%02x", (int)c);
        esc += 4;
        n -= 4;
      }
      r += 4;
    }
  }
  return r;
}

#define CSS_INJECT_FUNCTION                                                    \
  "(function(e){var "                                                          \
  "t=document.createElement('style'),d=document.head||document."               \
  "getElementsByTagName('head')[0];t.setAttribute('type','text/"               \
  "css'),t.styleSheet?t.styleSheet.cssText=e:t.appendChild(document."          \
  "createTextNode(e)),d.appendChild(t)})"


int webview_inject_css(webview_t w, const char *css) {
  int n = webview_js_encode(css, NULL, 0);
  char *esc = (char *)calloc(1, sizeof(CSS_INJECT_FUNCTION) + n + 4);
  if (esc == NULL) {
    return -1;
  }
  char *js = (char *)calloc(1, n);
  webview_js_encode(css, js, n);
  snprintf(esc, sizeof(CSS_INJECT_FUNCTION) + n + 4, "%s(\"%s\")",
           CSS_INJECT_FUNCTION, js);
  int r = webview_eval(w, esc);
  free(js);
  free(esc);
  return r;
}

#ifdef __cplusplus
}
#endif