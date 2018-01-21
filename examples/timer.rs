// #![windows_subsystem = "windows"]
#![allow(deprecated)]

extern crate url;
extern crate webview;

use std::thread::{spawn, sleep_ms};
use std::sync::{Arc, Mutex};
use url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET};
use webview::*;

fn main() {
	let size = (800, 600);
	let resizable = true;
	let debug = true;
	let userdata = 0;
	let counter = Arc::new(Mutex::new(0));
	let encoded: String = utf8_percent_encode(HTML, PATH_SEGMENT_ENCODE_SET).collect();
	let url = "data:text/html,".to_string() + &encoded;
	let counter_inner = counter.clone();
	run("timer example", &url, Some(size), resizable, debug, move |webview| {
		let counter_inner = counter_inner.clone();
		spawn(move || {
			loop {
				{
					let mut counter = counter_inner.lock().unwrap();
					*counter += 1;
					let counter_inner2 = counter_inner.clone();
					webview.dispatch(move |webview, userdata| {
						*userdata -= 1;
						let counter = counter_inner2.lock().unwrap();
						render(webview, *counter, *userdata);
					});
				}
				sleep_ms(1000);
			}
		});
	}, move |webview, arg, userdata| {
		match arg {
			"reset" => {
				*userdata += 10;
				let mut counter = counter.lock().unwrap();
				*counter = 0;
				render(webview, *counter, *userdata);
			}
			"exit" => {
				webview.terminate();
			}
			_ => unimplemented!()
		}
	}, userdata);
}

fn render<'a, T>(webview: &mut WebView<'a, T>, counter: u32, userdata: i32) {
	println!("counter: {}, userdata: {}", counter, userdata);
	webview.eval(&format!("updateTicks({}, {})", counter, userdata));
}

const HTML: &'static str = r#"
<!doctype html>
<html>
	<body>
		<p id="ticks"></p>
		<button onclick="external.invoke('reset')">reset</button>
		<button onclick="external.invoke('exit')">exit</button>
		<script type="text/javascript">
			function updateTicks(n, u) {
				document.getElementById('ticks').innerHTML = 'ticks ' + n + '<br>' + 'userdata ' + u;
			}
		</script>
	</body>
</html>
"#;