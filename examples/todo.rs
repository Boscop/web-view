// #![windows_subsystem = "windows"]

#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate web_view;

use web_view::*;

fn main() {
	let html = format!(r#"
		<!doctype html>
		<html>
			<head>
				{styles}
			</head>
			<body>
				<!--[if lt IE 9]>
				<div class="ie-upgrade-container">
					<p class="ie-upgrade-message">Please, upgrade Internet Explorer to continue using this software.</p>
					<a class="ie-upgrade-link" target="_blank" href="https://www.microsoft.com/en-us/download/internet-explorer.aspx">Upgrade</a>
				</div>
				<![endif]-->
				<!--[if gte IE 9 | !IE ]> <!-->
				{scripts}
				<![endif]-->
			</body>
		</html>
		"#,
		styles = inline_style(include_str!("todo/styles.css")),
		scripts = inline_script(include_str!("todo/picodom.js")) + &inline_script(include_str!("todo/app.js")),
	);
	let size = (320, 480);
	let resizable = false;
	let debug = true;
	let init_cb = |_webview| {};
	let userdata = vec![];
	let (tasks, _) = run("Rust Todo App", Content::Html(html), Some(size), resizable, debug, init_cb, |webview, arg, tasks: &mut Vec<Task>| {
		use Cmd::*;
		match serde_json::from_str(arg).unwrap() {
			init => (),
			log { text } => println!("{}", text),
			addTask { name } => tasks.push(Task { name, done: false }),
			markTask { index, done } => tasks[index].done = done,
			clearDoneTasks => tasks.retain(|t| !t.done),
		}
		render(webview, tasks);
	}, userdata);
	println!("final state: {:?}", tasks);
}

fn render<'a, T>(webview: &mut WebView<'a, T>, tasks: &[Task]) {
	println!("{:#?}", tasks);
	webview.eval(&format!("rpc.render({})", serde_json::to_string(tasks).unwrap()));
}

#[derive(Debug, Serialize, Deserialize)]
struct Task {
	name: String,
	done: bool,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize)]
#[serde(tag = "cmd")]
pub enum Cmd {
	init,
	log { text: String },
	addTask { name: String },
	markTask { index: usize, done: bool },
	clearDoneTasks,
}

fn inline_style(s: &str) -> String {
	format!(r#"<style type="text/css">{}</style>"#, s)
}

fn inline_script(s: &str) -> String {
	format!(r#"<script type="text/javascript">{}</script>"#, s)
}