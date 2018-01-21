#![windows_subsystem = "windows"]

extern crate url;
extern crate webview;

use url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET};
use webview::*;

fn main() {
	let size = (320, 480);
	let resizable = false;
	let debug = true;
	let init_cb = |_| {};
	let userdata = ();
	let html = include_str!("todo-ps/dist/bundle.html");
	let encoded: String = utf8_percent_encode(html, PATH_SEGMENT_ENCODE_SET).collect();
	let url = "data:text/html,".to_string() + &encoded;
	run("Rust / PureScript - Todo App", &url, Some(size), resizable, debug, init_cb, |_, _, _| {}, userdata);
}
