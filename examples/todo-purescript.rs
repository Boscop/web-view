#![windows_subsystem = "windows"]

extern crate urlencoding;
extern crate web_view;

use urlencoding::encode;
use web_view::*;

fn main() {
	let size = (320, 480);
	let resizable = false;
	let debug = true;
	let init_cb = |_webview| {};
	let frontend_cb = |_webview: &mut _, _arg: &_, _userdata: &mut _| {};
	let userdata = ();
	let html = include_str!("todo-ps/dist/bundle.html");
	let url = "data:text/html,".to_string() + &encode(html);
	run("Rust / PureScript - Todo App", &url, Some(size), resizable, debug, init_cb, frontend_cb, userdata);
}
