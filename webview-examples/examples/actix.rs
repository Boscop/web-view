#![windows_subsystem = "windows"]

use actix_web::{body::Body, dev::Server, rt, web, App, HttpRequest, HttpResponse, HttpServer};
use mime_guess::from_path;
use rust_embed::RustEmbed;
use std::{borrow::Cow, sync::mpsc, thread};
use web_view::*;

#[derive(RustEmbed)]
#[folder = "examples/actix"]
struct Asset;

fn assets(req: HttpRequest) -> HttpResponse {
    let path = if req.path() == "/" {
        // if there is no path, return default file
        "index.html"
    } else {
        // trim leading '/'
        &req.path()[1..]
    };

    // query the file from embedded asset with specified path
    match Asset::get(path) {
        Some(content) => {
            let body: Body = match content {
                Cow::Borrowed(bytes) => bytes.into(),
                Cow::Owned(bytes) => bytes.into(),
            };
            HttpResponse::Ok()
                .content_type(from_path(path).first_or_octet_stream().as_ref())
                .body(body)
        }
        None => HttpResponse::NotFound().body("404 Not Found"),
    }
}

fn run_actix(server_tx: mpsc::Sender<Server>, port_tx: mpsc::Sender<u16>) -> std::io::Result<()> {
    let server = rt::System::new();

    server.block_on(async {
        let server = HttpServer::new(|| App::new().service(web::resource("*").to(assets)))
            .bind("127.0.0.1:0")?;

        // we specified the port to be 0,
        // meaning the operating system
        // will choose some available port
        // for us
        // get the first bound address' port,
        // so we know where to point webview at
        let port = server.addrs().first().unwrap().port();
        port_tx.send(port).unwrap();

        let server = server.run();
        server_tx.send(server.clone()).unwrap();
        server.await
    })
}

fn main() {
    let (server_tx, server_rx) = mpsc::channel();
    let (port_tx, port_rx) = mpsc::channel();

    // start actix web server in separate thread
    thread::spawn(move || run_actix(server_tx, port_tx).unwrap());

    let port = port_rx.recv().unwrap();
    let server = server_rx.recv().unwrap();

    // start web view in current thread
    // and point it to a port that was bound
    // to actix web server
    web_view::builder()
        .title("Actix webview example")
        .content(Content::Url(format!("http://127.0.0.1:{}", port)))
        .size(400, 400)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(|_webview, _arg| Ok(()))
        .run()
        .unwrap();

    // gracefully shutdown actix web server
    rt::System::new().block_on(server.stop(true));
}
