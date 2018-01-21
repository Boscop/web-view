// #![windows_subsystem = "windows"]

extern crate url;
extern crate webview;

use url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET};
use webview::*;

fn main() {
	let size = (800, 600);
	let resizable = true;
	let debug = true;
	let init_cb = |_| {};
	let userdata = ();
	let encoded: String = utf8_percent_encode(HTML, PATH_SEGMENT_ENCODE_SET).collect();
	let url = "data:text/html,".to_string() + &encoded;
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