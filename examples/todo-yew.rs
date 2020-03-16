#![windows_subsystem = "windows"]

extern crate actix_rt;
extern crate actix_web;
extern crate futures;
extern crate mime_guess;
extern crate rust_embed;
extern crate web_view;

use actix_web::{body::Body, web, App, HttpRequest, HttpResponse, HttpServer};
use futures::future::Future;
use mime_guess::from_path;
use rust_embed::RustEmbed;
use std::{borrow::Cow, sync::mpsc, thread};
use web_view::*;

#[derive(RustEmbed)]
#[folder = "examples/todo-yew/static"]
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

fn main() {
    let (server_tx, server_rx) = mpsc::channel();
    let (port_tx, port_rx) = mpsc::channel();

    // start actix web server in separate thread
    thread::spawn(move || {
        let sys = actix_rt::System::new("actix-example");

        let server = HttpServer::new(|| App::new().route("*", web::get().to(assets)))
            .bind("127.0.0.1:0")
            .unwrap();

        // we specified the port to be 0,
        // meaning the operating system
        // will choose some available port
        // for us
        // get the first bound address' port,
        // so we know where to point webview at
        let port = server.addrs().first().unwrap().port();
        let server = server.start();

        let _ = port_tx.send(port);
        let _ = server_tx.send(server);
        let _ = sys.run();
    });

    let port = port_rx.recv().unwrap();
    let server = server_rx.recv().unwrap();

    // start web view in current thread
    // and point it to a port that was bound
    // to actix web server
    web_view::builder()
        .title("todomvc example")
        .content(Content::Url(format!("http://127.0.0.1:{}", port)))
        .size(600, 500)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(|_webview, _arg| Ok(()))
        .run()
        .unwrap();

    // gracefully shutdown actix web server
    let _ = server.stop(true).wait();
}
