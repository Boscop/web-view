//#![windows_subsystem = "windows"]

extern crate web_view;

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
                "open" => match webview.dialog().open_file("Please choose a file...", "")? {
                    Some(path) => webview.dialog().info("File chosen", path.to_string_lossy()),
                    None => webview
                        .dialog()
                        .warning("Warning", "You didn't choose a file."),
                }?,
                "save" => match webview.dialog().save_file()? {
                    Some(path) => webview.dialog().info("File chosen", path.to_string_lossy()),
                    None => webview
                        .dialog()
                        .warning("Warning", "You didn't choose a file."),
                }?,
                "info" => webview.dialog().info("Info", "This is a info dialog")?,
                "warning" => webview
                    .dialog()
                    .warning("Warning", "This is a warning dialog")?,
                "error" => webview.dialog().error("Error", "This is a error dialog")?,
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
