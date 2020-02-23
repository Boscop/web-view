extern crate web_view;

use web_view::*;

fn main() {
    let res = web_view::builder()
        .title("Graceful Exit Example")
        .content(Content::Html(include_str!("graceful_exit/index.html")))
        .size(800, 600)
        .resizable(true)
        .debug(true)
        .user_data(0)
        .invoke_handler(invoke_handler)
        .run()
        .unwrap();
    println!("res: {:?}", res)
}

fn invoke_handler(wv: &mut WebView<usize>, arg: &str) -> WVResult {
    if arg == "init" {
        wv.eval("init()")?;
    } else if arg == "update" {
        *wv.user_data_mut() += 1;
        let js = format!("setCurrentCount({})", wv.user_data());
        wv.eval(&js)?;
    } else if arg == "exit" {
        println!("exiting!");
        wv.exit();
    }
    Ok(())
}
