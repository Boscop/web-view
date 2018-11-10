#![windows_subsystem = "windows"]

extern crate web_view;

use web_view::*;

fn main() {
    WebViewBuilder::new()
        .title("Page load example")
        .content(Content::Html(HTML))
        .size(800, 600)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(|_webview, _arg| Ok(()))
        .build()
        .unwrap()
        .run()
        .unwrap();
}

const HTML: &str = r#"
<!doctype html>
<html>
	<body>
	  <h1>Hello, world</h1>
	</body>
</html>
"#;
