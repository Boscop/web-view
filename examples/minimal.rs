// #![windows_subsystem = "windows"]

extern crate web_view;

use web_view::*;

fn main() {
	let size = (800, 600);
	let resizable = true;
	let debug = true;
	let init_cb = |_| {};
	let userdata = ();
	run(
		"Minimal webview example",
		"https://en.m.wikipedia.org/wiki/Main_Page",
		Some(size),
		resizable,
		debug,
		init_cb,
		/* frontend_cb: */ |_, _, _| {},
		userdata
	);
}