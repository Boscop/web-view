extern crate cc;
extern crate pkg_config;

use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
	// Initialize nanovg submodule if user forgot to clone parent repository with --recursive.
    if !Path::new("webview/.git").exists() {
        let _ = Command::new("git").args(&["submodule", "update", "--init"])
                                   .status();
	}

	let mut build = cc::Build::new();
	build.flag_if_supported("-std=c11");
	build.include("webview");
	build.file("webview.c");
	if env::var("DEBUG").is_err() {
		build.define("NDEBUG", None);
	} else {
		build.define("DEBUG", None);
	}
	let target = env::var("TARGET").unwrap();
	if target.contains("windows") {
		build.define("WEBVIEW_WINAPI", None);
		for &lib in &["ole32", "comctl32", "oleaut32", "uuid", "gdi32"] {
			println!("cargo:rustc-link-lib={}", lib);
		}
	} else if target.contains("linux") || target.contains("bsd") {
		let webkit = pkg_config::Config::new().atleast_version("2.8").probe("webkit2gtk-4.0").unwrap();

		for path in webkit.include_paths {
			build.include(path);
		}
		build.define("WEBVIEW_GTK", None);
	} else if target.contains("apple") {
		build.define("WEBVIEW_COCOA", None);
		build.flag("-x");
		build.flag("objective-c");
		println!("cargo:rustc-link-lib=framework=Cocoa");
		println!("cargo:rustc-link-lib=framework=WebKit");
	} else {
		panic!("unsupported target");
	}
	build.compile("webview");
}
