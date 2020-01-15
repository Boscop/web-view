extern crate web_view;

use web_view::*;

fn main() {
    web_view::builder()
        .title("Frameless example")
        .content(Content::Html(HTML))
        .size(800, 800)
        .frameless(true)
        .debug(true)
        .user_data("")
        .invoke_handler(|webview, arg| {
            match arg {
                "exit" => webview.exit(),
                _ => ()
            }
            Ok(())
        })
        .run()
        .unwrap();
}

const HTML: &str = r#"
<!doctype html>
<html>
	<body style="width: 800px;height:800px;">
        <button onclick="external.invoke('exit')" style="display:block;width:100px;height:100px;font-size:24pt;margin:375px auto;">exit</button>
	</body>
</html>
"#;
