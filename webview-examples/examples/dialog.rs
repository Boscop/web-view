//#![windows_subsystem = "windows"]

extern crate tinyfiledialogs as tfd;
extern crate web_view;

use tfd::MessageBoxIcon;
use web_view::*;

fn main() -> WVResult {
    let webview = web_view::builder()
        .title("Dialog example")
        .content(Content::Html(HTML))
        .size(800, 600)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(|webview, arg| {
            match arg {
                "open" => match tfd::open_file_dialog("Please choose a file...", "", None) {
                    Some(path) => tfd::message_box_ok("File chosen", &path, MessageBoxIcon::Info),
                    None => tfd::message_box_ok(
                        "Warning",
                        "You didn't choose a file.",
                        MessageBoxIcon::Warning,
                    ),
                },
                "save" => match tfd::save_file_dialog("Save file...", "") {
                    Some(path) => tfd::message_box_ok("File chosen", &path, MessageBoxIcon::Info),
                    None => tfd::message_box_ok(
                        "Warning",
                        "You didn't choose a file.",
                        MessageBoxIcon::Warning,
                    ),
                },
                "info" => {
                    tfd::message_box_ok("Info", "This is a info dialog", MessageBoxIcon::Info)
                }
                "warning" => tfd::message_box_ok(
                    "Warning",
                    "This is a warning dialog",
                    MessageBoxIcon::Warning,
                ),
                "error" => {
                    tfd::message_box_ok("Error", "This is a error dialog", MessageBoxIcon::Error)
                }
                "exit" => webview.exit(),
                _ => unimplemented!(),
            };
            Ok(())
        })
        .build()?;

    webview.run()
}

const HTML: &str = r#"
<!doctype html>
<html>
    <body>
        <button onclick="external.invoke('open')">Open</button>
        <button onclick="external.invoke('save')">Save</button>
        <button onclick="external.invoke('info')">Info</button>
        <button onclick="external.invoke('warning')">Warning</button>
        <button onclick="external.invoke('error')">Error</button>
        <button onclick="external.invoke('exit')">Exit</button>
    </body>
</html>
"#;
