extern crate cc;
extern crate pkg_config;

use std::env;

fn main() {
    let mut build = cc::Build::new();

    let target = env::var("TARGET").unwrap();

    build
        .include("webview.h")
        .flag_if_supported("-std=c11")
        .flag_if_supported("-w");

    if env::var("DEBUG").is_err() {
        build.define("NDEBUG", None);
    } else {
        build.define("DEBUG", None);
    }

    if target.contains("windows") {
        build.define("UNICODE", None);

        if cfg!(feature = "edge") {
            build
                .file("webview_edge.cpp")
                .flag_if_supported("/std:c++17");

            for &lib in &["windowsapp", "user32", "gdi32", "ole32"] {
                println!("cargo:rustc-link-lib={}", lib);
            }
        } else {
            build.file("webview_mshtml.c");

            for &lib in &["ole32", "comctl32", "oleaut32", "uuid", "gdi32", "user32"] {
                println!("cargo:rustc-link-lib={}", lib);
            }
        }
    } else if target.contains("linux") || target.contains("bsd") {
        pkg_config::Config::new()
            .atleast_version("2.8")
            .probe("webkit2gtk-4.0")
            .unwrap();
    } else if target.contains("apple") {
        build
            .file("webview_cocoa.c")
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
