extern crate web_view;

use web_view::*;

fn main() {

    web_view::builder()
        .title("Change text")
        .content(Content::Html(HTML))
        .size(200, 100)
        .resizable(true)
        .debug(true)
        .user_data("")
        .invoke_handler(|webview, arg| {

			let tokens: Vec<&str> = arg.split(":").collect();
			let prefix = tokens[0];
			let data = tokens[1];

			match prefix {
				"toUppercase" => {
								let data_uc = to_uc(data);
								let to_uc_js = format!("toUppercase(\"{}\");", data_uc);
								webview.eval(&to_uc_js)?;
							 }
				_ => (),
			}

            Ok(())
        })
        .run()
        .unwrap();
}

fn to_uc(data: &str) -> String {
		data.to_uppercase()
}

const HTML: &str = r#"
<!doctype html>
<html>
	<head>
		<meta http-equiv="X-UA-Compatible" content="IE=edge">
		<script type="text/javascript">
			function changeText() {
				var textOne = document.getElementById("textOne");
				external.invoke('toUppercase:' + textOne.value);
			}
			function toUppercase(data) {
				var textTwo = document.getElementById("textTwo");
				textTwo.value = data;
			}
		</script>
	</head>
	<body>
		<input id="textOne" type="text" />
		<input id="textTwo" type="text" />
		<button onclick="changeText()">Change Text</button>
	</body>
</html>
"#;

