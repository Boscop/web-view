#include "webview.h"

#include <JavaScriptCore/JavaScript.h>
#include <gtk/gtk.h>
#include <webkit2/webkit2.h>

struct gtk_webview {
  const char *url;
  const char *title;
  int width;
  int height;
  int resizable;
  int debug;
  webview_external_invoke_cb_t external_invoke_cb;
  GtkWidget *window;
  GtkWidget *scroller;
  GtkWidget *webview;
  GtkWidget *inspector_window;
  GAsyncQueue *queue;
  int ready;
  int js_busy;
  int should_exit;
  void *userdata;
};

void webview_destroy_cb(GtkWidget *widget, gpointer arg) {
  (void)widget;
  webview_terminate((webview_t)arg);
}

WEBVIEW_API void webview_terminate(webview_t w) {
  struct gtk_webview *wv = (struct webview *)w;
  wv->should_exit = 1;
}

WEBVIEW_API void webview_exit(webview_t w) { (void)w; }
WEBVIEW_API void webview_print_log(const char *s) {
  fprintf(stderr, "%s\n", s);
}
