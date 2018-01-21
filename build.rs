extern crate gcc;

use std::env;

fn main() {
	let mut build = gcc::Build::new();
	build.cpp(true).file("webview-c/lib.c").include("webview-c");
	if env::var("DEBUG").is_err() {
		build.define("NDEBUG", None);
	} else {
		build.define("DEBUG", None);
	}
	let target = env::var("TARGET").unwrap();
	if target.contains("windows") {
		build.define("WEBVIEW_WINAPI", None);
	} else if target.contains("linux") || target.contains("bsd") {
		build.define("WEBVIEW_GTK", None);
	} else if target.contains("apple") {
		build.define("WEBVIEW_COCOA", None);
	} else {
		panic!("unsupported target");
	}
	build.compile("libwebview.a");
}
