extern crate web_view;

use web_view::*;

fn main() {
    web_view::builder()
        .title("Frameless example")
        .content(Content::Html(HTML))
        .size(150, 150)
        .frameless(true)
        .debug(true)
        .user_data("")
        .invoke_handler(|webview, arg| {
            match arg {
                "exit" => webview.exit(),
                _ => (),
            }
            Ok(())
        })
        .run()
        .unwrap();
}

const HTML: &str = r#"
<!doctype html>
<html>
	<body>
        <button onclick="external.invoke('exit')" style="display:block;width:100px;height:100px;font-size:24pt;margin:25px auto;">exit</button>
	</body>
</html>
"#;
