extern crate web_view;

use web_view::*;

fn main() {
    println!("starting graceful exit example");
    let after = web_view::builder()
        .title("Gracefully exiting webview example")
        .content(Content::Html(create_html()))
        .size(800, 600)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(invoke_handler)
        .run()
        .unwrap();
    println!("after exit: {:?}", after);
}

fn invoke_handler(wv: &mut WebView<()>, arg: &str) -> WVResult {
    println!("in handler: {}", arg);
    if arg == "true" {
        wv.queue_close()
    } else {
        Ok(())
    }
}

fn create_html() -> String {
    "<!DOCTYPE html>
    <html>
        <head></head>
        <body>
            <button onclick=\"window.external.invoke('true')\">Click me to exit</button>
        </body>
    </html>"
        .to_string()
}
