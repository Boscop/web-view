extern crate cc;
extern crate pkg_config;

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let webview_path: PathBuf = match env::var("WEBVIEW_DIR") {
        Ok(path) => path.into(),
        Err(_) => {
            // Initialize webview submodule if user forgot to clone parent repository with --recursive.
            if !Path::new("webview/.git").exists() {
                let _ = Command::new("git")
                    .args(&["submodule", "update", "--init"])
                    .status();
            }
            "webview".into()
        }
    };

    let mut build = cc::Build::new();

    build
        .include(&webview_path)
        .file("webview.c")
        .flag_if_supported("-std=c11")
        .flag_if_supported("-Wno-everything");

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
            .flag("-x")
            .flag("objective-c");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=WebKit");
    } else {
        panic!("unsupported target");
    }

    build.compile("webview");
}
