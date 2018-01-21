// #![windows_subsystem = "windows"]

extern crate urlencoding;
extern crate webview;

use urlencoding::encode;
use webview::*;

fn main() {
	let size = (800, 600);
	let resizable = true;
	let debug = true;
	let init_cb = |_| {};
	let userdata = ();
	let url = "data:text/html,".to_string() + &encode(HTML);
	run("pageload example", &url, Some(size), resizable, debug, init_cb, |_, _, _| {}, userdata);
}

const HTML: &'static str = r#"
<!doctype html>
<html>
	<body>
	  <h1>Hello, world</h1>
	</body>
</html>
"#;