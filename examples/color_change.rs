extern crate web_view;

use web_view::*;

fn main() {
    web_view::builder()
        .title("Change background color")
        .content(Content::Html(HTML))
        .size(200, 100)
        .resizable(true)
        .debug(true)
        .user_data("")
        .invoke_handler(|webview, arg| {
            match arg {
                "red" => webview.set_color((255, 0, 0)),
                "green" => webview.set_color((0, 255, 0)),
                "blue" => webview.set_color((0, 0, 255)),
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
		<button onclick="external.invoke('red')">red</button>
        <button onclick="external.invoke('green')">green</button>
        <button onclick="external.invoke('blue')">blue</button>
	</body>
</html>
"#;
