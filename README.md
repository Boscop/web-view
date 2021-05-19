# web-view &emsp; ![.github/workflows/ci.yml](https://github.com/Boscop/web-view/workflows/.github/workflows/ci.yml/badge.svg) [![Latest Version]][crates.io] <!-- omit in toc -->

[Build Status]: https://api.travis-ci.org/Boscop/web-view.svg?branch=master
[travis]: https://travis-ci.org/Boscop/web-view
[Latest Version]: https://img.shields.io/crates/v/web-view.svg
[crates.io]: https://crates.io/crates/web-view 

- [Prerequisites](#prerequisites)
- [Installation and Configuration](#installation-and-configuration)
- [Known Issues and Limitations](#known-issues-and-limitations)
- [Suggestions](#suggestions)
- [Contribution opportunities](#contribution-opportunities)
- [Ideas for apps](#ideas-for-apps)
- [Showcase](#showcase)

> **Important:** requires Rust 1.30 stable or newer.

This library provides a Rust binding to the original implementation of [webview](https://github.com/zserge/webview), a tiny cross-platform library to render web-based GUIs as desktop applications.

<p align="center"><img alt="screenshot" src="https://i.imgur.com/Z3c2zwD.png"></p>

Two-way binding between your Rust and JavaScript code is made simple via the `external` JS object and `webview.eval` Rust function. We have full [working examples](https://github.com/Boscop/web-view/tree/master/webview-examples/examples), but the core is as follows:
 
```rust
// ... Simplified for the sake of brevity.
web_view::builder()    
    .invoke_handler(|webview, arg| {
        match arg {
            "test_one" => {
                // Do something in Rust!
            }
            "test_two" => {
                // Invoke a JavaScript function!
                webview.eval(&format!("myFunction({}, {})", 123, 456))
            }
            _ => unimplemented!(),
        };
    })
```
 
```javascript
// Executes our "invoke_handler" - passing the value "test_one" as the second parameter.
external.invoke('test_one');

// Executes our "invoke_handler", which in turn calls "myFunction" below.
external.invoke('test_two');

function myFunction(paramOne, paramTwo) {
    console.log(paramOne);
    console.log(paramTwo);
}
```
 
In addition, by relying on the default rendering engine of the host Operating System, you should be met with a *significantly* leaner binary to distribute compared to alternatives such as [Electron](https://github.com/electron/electron) which have to bundle Chromium with each distribution. 
 
> *You should also see comparatively less memory usage, and this section will be updated with benchmarks to highlight this in due course.*
 
Finally, the supported platforms and the engines you can expect to render your application content are as follows:
 
| Operating System | Browser Engine Used |
| ---------------- | ------------------- |
| Windows          | MSHTML or EdgeHTML  |
| Linux            | Gtk-webkit2         |
| OSX              | Cocoa/WebKit        |
 
> Note: by default the MSHTML (IE) rendering engine is used to display the application on Windows. If you want to make use of EdgeHTML (Edge) then you'll need to enable it with a feature switch (see the [installation and configuration section](#installation-and-configuration)).
 
## Prerequisites
 
If you're planning on targeting Linux you **must** ensure that `Webkit2gtk` is already installed and available for discovery via the [pkg-config](https://linux.die.net/man/1/pkg-config) command.
 
If you skip this step you will see a similarly formatted error message as below informing you of what's missing:
 
```text
Compiling webview-sys v0.3.3
error: failed to run custom build command for `webview-sys v0.3.3`
Caused by:
process didn't exit successfully: `/home/username/rust-projects/my-project/target/debug/build/webview-sys-9020ddaf41e4df7d/build-script-build` (exit code: 101)
--- stderr
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Command { command: "\"pkg-config\" \"--libs\" \"--cflags\" \"webkit2gtk-4.0\" \"webkit2gtk-4.0 >= 2.8\"", cause: Os { code: 2, kind: NotFound, message: "No such file or directory" } }', src/libcore/result.rs:1165:5
```

## Installation and Configuration

Let's start off with the basic Rust application. Run `cargo new my-project` in a shell of your choice and change into the `my-project` directory.

As this library can be found as a crate on the [Rust Community Registry](https://crates.io/crates/web-view) all you have to do to add this as a dependency is update your `Cargo.toml` file to have the following under its dependencies section:

```toml
[dependencies]
web-view = { version = "0.7" }
```

If you want to make use of **Edge** on Windows environments, you will need to have Windows 10 SDK installed through Visual Studio Installer and you'll need to use the following syntax instead:

```toml
[dependencies]
web-view = { version = "0.7", features = ["edge"] }
```

Now let's write some Rust code that makes use of the library. Open up the `main.rs` file in an editor of your choice:

```bash
vim src/main.rs
```

And replace the contents with the following:

```rust
use web_view::*;

fn main() {
    let html_content = "<html><body><h1>Hello, World!</h1></body></html>";
	
    web_view::builder()
        .title("My Project")
        .content(Content::Html(html_content))
        .size(320, 480)
        .resizable(false)
        .debug(true)
        .user_data(())
        .invoke_handler(|_webview, _arg| Ok(()))
        .run()
        .unwrap();
}
```

You should now be able to run `cargo build` and see something similar to the output below:

```text
$ cargo build
Updating crates.io index
Compiling pkg-config v0.3.17
Compiling cc v1.0.47
Compiling boxfnonce v0.1.1
Compiling urlencoding v1.0.0
Compiling webview-sys v0.3.3
Compiling web-view v0.5.4
Compiling my-project v0.1.0 (C:\Users\Username\source\rust-projects\my-project)
Finished dev [unoptimized + debuginfo] target(s) in 8.36s
```
 
Assuming you get a successful build all you have to do now is run it with: `cargo run`. Hopefully you'll see the same as below:

<p align="center"><img alt="screenshot" src="https://i.imgur.com/vQrS2p2.png"></p>

For more usage info please check out the [examples](https://github.com/Boscop/web-view/tree/master/webview-examples/examples) and the [original README](https://github.com/zserge/webview/blob/master/README.md).

## Known Issues and Limitations
 
* `Edge` feature switch not working on Windows 10 if run as `Administrator`. This was the root cause of the issue raised in [#96](https://github.com/Boscop/web-view/issues/96) and is the result of a bug in `Microsoft.Toolkit.Win32` which is [tracked here](https://github.com/windows-toolkit/Microsoft.Toolkit.Win32/issues/50).
* `Edge` sandbox restrictions. If you decide to make use of an embedded Web Server to return your content you will need to run the following command to bypass the restriction that prevents communication with `localhost`:
 
    ``` powershell
    $ # Requires administrative privileges.
    $ CheckNetIsolation.exe LoopbackExempt -a -n="Microsoft.Win32WebViewHost_cw5n1h2txyewy"
    ```
 
    This is usually used with Windows IoT Core, when allowing TCP/IP connections between two processes. You can read some more about this in the [Microsoft Documentation here](https://docs.microsoft.com/en-us/windows/iot-core/develop-your-app/loopback).
 
* `IE` rendering content in a legacy, compatibility format. By default, content rendered inside a Web Browser Control will be done so in compatibility mode ([specifically IE7](https://docs.microsoft.com/en-us/previous-versions/windows/internet-explorer/ie-developer/general-info/ee330730(v=vs.85)?redirectedfrom=MSDN)). To get round this on Windows systems where Edge is not available you can force the use of the highest version of IE installed via a [Registry tweak](https://blogs.msdn.microsoft.com/patricka/2015/01/12/controlling-webbrowser-control-compatibility/).
 
## Suggestions
 
- If you like type safety, write your frontend in [Elm](http://elm-lang.org/) or [PureScript](http://www.purescript.org/)<sup>[*](#n1)</sup>, or use a Rust frontend framework that compiles to asm.js, like [yew](https://github.com/DenisKolodin/yew).
- Use [parcel](https://parceljs.org/) to bundle & minify your frontend code.
- Use [inline-assets](https://www.npmjs.com/package/inline-assets) to inline all your assets (css, js, html) into one index.html file and embed it in your Rust app using `include_str!()`.
- If your app runs on windows, [add an icon](https://github.com/mxre/winres) to your Rust executable to make it look more professional™
- Use custom npm scripts or [just](https://github.com/casey/just) or [cargo-make](https://github.com/sagiegurari/cargo-make) to automate the build steps.
- Make your app state persistent between sessions using localStorage in the frontend or [rustbreak](https://crates.io/crates/rustbreak) in the backend.
- Btw, instead of injecting app resources via the js api, you can also serve them from a local http server (e.g. bound to an ephemeral port).
- Happy coding :)
 
<a name="n1">*</a> The free [PureScript By Example](https://leanpub.com/purescript/read) book contains several practical projects for PureScript beginners.
 
## Contribution opportunities
 
- Create an issue for any question you have
- Docs
- Feedback on this library's API and code
- Test it on non-windows platforms, report any issues you find
- Showcase your app
- Add an example that uses Elm or Rust compiled to asm.js
- Add a PureScript example that does two-way communication with the backend
- Contribute to the original webview library: E.g. [add HDPI support on Windows](https://github.com/zserge/webview/issues/54)
- Make it possible to create the webview window as a child window of a given parent window. This would allow webview to be used for the GUIs of [VST audio plugins in Rust](https://github.com/rust-dsp/rust-vst).
 
## Ideas for apps
 
- Rust IDE (by porting [xi-electron](https://github.com/acheronfail/xi-electron) to web-view)
- Data visualization / plotting lib for Rust, to make Rust more useful for data science
- Crypto coin wallet
- IRC client, or client for other chat protocols
- Midi song editor, VJ controller
- Rust project template wizard: Generate new Rust projects from templates with user-friendly steps
- GUI for [pijul](https://pijul.org/)
- Implement [Gooey](https://github.com/chriskiehl/Gooey) alternative with [web-view](https://github.com/Boscop/web-view) and [clap-rs](https://github.com/clap-rs/clap)
 
## Showcase
 
*Feel free to open a PR if you want your project to be listed here!*  
 
- [Juggernaut](https://github.com/ShashankaNataraj/Juggernaut) - The unstoppable programmers editor
- [FrakeGPS](https://github.com/frafra/frakegps) - Simulate a simple GPS device
- [Compactor](https://github.com/Freaky/Compactor) - Windows 10 filesystem compression utility
- [neutrino](https://github.com/alexislozano/neutrino/) - A GUI frontend in Rust based on web-view
- [SOUNDSENSE-RS](https://github.com/prixt/soundsense-rs) - Sound-engine tool for Dwarf Fortress
- [WV Linewise](https://github.com/forbesmyester/wv-linewise) - Add your own interactive HTML/CSS/JS in the middle of your UNIX pipelines
- [Bloop](https://github.com/Blakeinstein/Bloop) - A light weight aesthetic scratchpad for developers.

---

Contributions and feedback welcome :)

---
