#![windows_subsystem = "windows"]

extern crate web_view;

use web_view::*;

fn main() {
	let size = (320, 480);
	let resizable = false;
	let debug = true;
	let init_cb = |_webview| {};
	let frontend_cb = |_webview: &mut _, _arg: &_, _userdata: &mut _| {};
	let userdata = ();
	let html = include_str!("elm-counter/index.html");
	run("Rust / Elm - Counter App", Content::Html(html), Some(size), resizable, debug, init_cb, frontend_cb, userdata);
}
