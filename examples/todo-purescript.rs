#![windows_subsystem = "windows"]

extern crate urlencoding;
extern crate webview;

use urlencoding::encode;
use webview::*;

fn main() {
	let size = (320, 480);
	let resizable = false;
	let debug = true;
	let init_cb = |_| {};
	let userdata = ();
	let html = include_str!("todo-ps/dist/bundle.html");
	let url = "data:text/html,".to_string() + &encode(html);
	run("Rust / PureScript - Todo App", &url, Some(size), resizable, debug, init_cb, |_, _, _| {}, userdata);
}
