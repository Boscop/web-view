// #![windows_subsystem = "windows"]
#![allow(deprecated)]

extern crate web_view;

use std::thread::{spawn, sleep_ms};
use std::sync::{Arc, Mutex};
use web_view::*;

fn main() {
	let size = (800, 600);
	let resizable = true;
	let debug = true;
	let initial_userdata = 0;
	let counter = Arc::new(Mutex::new(0));
	let counter_inner = counter.clone();
	run("timer example", Content::Html(HTML), Some(size), resizable, debug, move |webview| {
		spawn(move || {
			loop {
				{
					let mut counter = counter_inner.lock().unwrap();
					*counter += 1;
					webview.dispatch(|webview, userdata| {
						*userdata -= 1;
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
	}, initial_userdata);
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