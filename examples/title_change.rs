extern crate web_view;

use web_view::*;

fn main() {
    web_view::builder()
        .title("Change title with input")
        .content(Content::Html(HTML))
        .size(800, 100)
        .resizable(true)
        .debug(true)
        .user_data("")
        .invoke_handler(|webview, arg| webview.set_title(arg))
        .run()
        .unwrap();
}

const HTML: &str = r#"
<!doctype html>
<html>
	<body>
        <input id="title" type="text" placeholder="Title" style="width: 100%; padding: none; margin: none; font-size: large;"/>
        <script type="text/javascript">
            function updateTitle(e) {
                external.invoke(e.target.value);
            }
            var el = document.getElementById("title");
            el.addEventListener("input", updateTitle, false);
		</script>
	</body>
</html>
"#;
