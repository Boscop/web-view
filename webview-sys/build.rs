extern crate cc;
extern crate pkg_config;

use std::env;

fn main() {
    let mut build = cc::Build::new();

    if target.contains("windows") && cfg!(feature = "edge") {
        build
            .include("webview_edge.h")
            .file("webview_edge.cc")
            .flag_if_supported("/std:c++17");
    } else {
        build
            .include("webview.h")
            .file("webview.c")
            .flag_if_supported("-std=c11")
            .flag_if_supported("-w");
    }

    if env::var("DEBUG").is_err() {
        build.define("NDEBUG", None);
    } else {
        build.define("DEBUG", None);
    }

    let target = env::var("TARGET").unwrap();

    if target.contains("windows") {
        if !cfg!(feature = "edge") {
            build.define("WEBVIEW_WINAPI", None);
            for &lib in &["ole32", "comctl32", "oleaut32", "uuid", "gdi32"] {
                println!("cargo:rustc-link-lib={}", lib);
            }
        }
    } else if target.contains("linux") || target.contains("bsd") {
        let webkit = pkg_config::Config::new()
            .atleast_version("2.8")
            .probe("webkit2gtk-4.0")
            .unwrap();

        for path in webkit.include_paths {
            build.include(path);
        }
        build.define("WEBVIEW_GTK", None);
    } else if target.contains("apple") {
        build
            .define("WEBVIEW_COCOA", None)
            .define("OBJC_OLD_DISPATCH_PROTOTYPES", "1")
            .flag("-x")
            .flag("objective-c");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=WebKit");
    } else {
        panic!("unsupported target");
    }

    build.compile("webview");
}
