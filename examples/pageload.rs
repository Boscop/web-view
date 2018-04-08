// #![windows_subsystem = "windows"]

extern crate web_view;

use web_view::*;

fn main() {
	let size = (800, 600);
	let resizable = true;
	let debug = true;
	let init_cb = |_webview| {};
	let frontend_cb = |_webview: &mut _, _arg: &_, _userdata: &mut _| {};
	let userdata = ();
	run("pageload example", Content::Html(HTML), Some(size), resizable, debug, init_cb, frontend_cb, userdata);
}

const HTML: &'static str = r#"
<!doctype html>
<html>
	<body>
	  <h1>Hello, world</h1>
	</body>
</html>
"#;