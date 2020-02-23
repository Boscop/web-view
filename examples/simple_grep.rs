extern crate grep;
extern crate walkdir;
extern crate web_view;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use std::error::Error;
use std::ffi::OsString;
use walkdir::WalkDir;
use web_view::*;

fn main() {
    web_view::builder()
        .title("Simple Grep Example")
        .content(Content::Html(HTML))
        .size(825, 625)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(|webview, arg| {
            use Cmd::*;

            match serde_json::from_str(arg).unwrap() {
                Search { pattern, path } => {
                    let result = match search(&pattern, OsString::from(path), webview) {
                        Ok(s) => s,
                        Err(err) => {
                            let err_str = format!("{}", err);
                            webview.dialog().error("Error", err_str)?;
                            OsString::from("")
                        }
                    };
                    if result.is_empty() {
                        webview
                            .dialog()
                            .info("Information", "No results were found!")?;
                    } else {
                        let eval_str = format!("LoadTextArea({:?});", result);
                        webview.eval(&eval_str)?;
                    }
                }

                Browse {} => match webview.dialog().open_file("Please choose a file...", "")? {
                    Some(path_selected) => {
                        let eval_str = format!("SetPath({:?});", path_selected.as_os_str());
                        webview.eval(&eval_str)?;
                    }
                    None => {
                        webview
                            .dialog()
                            .warning("Warning", "You didn't choose a file.")?;
                    }
                },

                Error { msg } => webview.dialog().error("Error", msg)?,
            }

            Ok(())
        })
        .run()
        .unwrap();
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum Cmd {
    Search { pattern: String, path: String },
    Browse {},
    Error { msg: String },
}

fn search(
    pattern: &str,
    path: OsString,
    webview: &mut WebView<()>,
) -> Result<OsString, Box<dyn Error>> {
    let matcher = RegexMatcher::new_line_matcher(&pattern)?;
    let mut matches: OsString = OsString::new();
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(true)
        .build();

    let mut matched_line = OsString::new();

    for result in WalkDir::new(path) {
        let entry = match result {
            Ok(entry) => entry,
            Err(err) => {
                let err_str = format!("{}", err);
                webview.dialog().error("Error", err_str)?;
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }

        match searcher.search_path(
            &matcher,
            entry.path(),
            UTF8(|lnum, line| {
                matched_line = OsString::from(format!(
                    "{:?}\t {}:\t {}",
                    entry.path(),
                    lnum.to_string(),
                    line.to_string()
                ));
                matches.push(&matched_line);
                matches.push("\n");
                Ok(true)
            }),
        ) {
            Ok(()) => (),
            Err(err) => {
                let err_str = format!("{}: {:?}", err, entry.path());
                webview.dialog().error("Error", err_str)?;
                continue;
            }
        }
    }

    Ok(matches)
}

const HTML: &str = r#"
<!doctype html>
<html>
	<head>
		<style>
			.textarea {
				width: 100%;
				height: 30em;
				font-size: 1em;				
			}
		</style>
		<script type="text/javascript">
			'use strict';
			var rpc = {
				invoke : function(arg) { window.external.invoke(JSON.stringify(arg)); },
				search : function() {
					var pattern = document.getElementById("pattern");
					var path = document.getElementById("path");
					if (pattern.value.trim().length === 0) {
						rpc.error("No pattern entered!");
						return;
					}
					if (path.value.trim().length === 0) {
						rpc.error("No path entered!");
						return;
					}
					var textArea = document.getElementById("text_box");
					textArea.value = "";
					rpc.invoke({cmd : 'search', path : path.value, pattern : pattern.value});
				},
				browse : function() { rpc.invoke({cmd : 'browse'}); },
				error : function(msg) { rpc.invoke({cmd : 'error', msg : msg}); },
			};
				
			function LoadTextArea(data) {
				var textArea = document.getElementById("text_box");
				textArea.value = data;
			}
			function SetPath(path_selected) {
				var path = document.getElementById("path");
				path.value = path_selected;
			}
		</script>
	</head>
	<body>
		<label for="pattern">Patern to search for:</label>
		<input style="font-size:16px" id="pattern" type="text" size="35" />
		<label style="font-size:14px"> (prefix text with "(?i)" to ignore case)</label><br><br>
		<label for="path">Path (directory or file):</label>
		<input id="path" type="text" size="70" />
	    <button onclick="rpc.browse()">Browse</button>
		<button onclick="rpc.search()">Search</button>
		<textarea class="textarea" id="text_box"></textarea>
	</body>
</html>
"#;
