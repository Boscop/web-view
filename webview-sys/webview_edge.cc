#include "webview_edge.h"

#ifdef __cplusplus
extern "C" {
#endif

void wrapper_webview_free(webview_t w) {
	webview_destroy(w);
}

webview_t wrapper_webview_new(const char* title, const char* url, int width, int height, int resizable, int debug, webview_external_invoke_cb_t external_invoke_cb, void* userdata) {
	webview_t w = webview_create(external_invoke_cb, width, height, resizable, debug);
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

void webview_set_color(webview_t w, uint8_t r, uint8_t g, uint8_t b, uint8_t a)
{
	// TODO
}

void webview_dialog(webview_t w, int dlgtype, int flags, const char *title, const char *arg, char *result, size_t resultsz)
{
	// TODO
}

void webview_set_fullscreen(webview_t w, int fullscreen)
{
	// TODO
}

int webview_inject_css(webview_t w, const char *css)
{
	// TODO
	return 0;
}

#ifdef __cplusplus
}
#endif