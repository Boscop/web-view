# web-view examples

## minimal
Just displays the wikipedia homepage.

## pageload
Loads a custom url-encoded html page (hello world).

## timer
Uses two-way communication with the web app to render the state of a timer and reset the timer on the click of a button. Shows basic usage of `userdata` and shared state between threads.

## todo
Uses picodom.js to render a basic Todo App. Demonstrates how to embed the frontend into the Rust executable and how to use `userdata` to store app state.

## todo-purescript
This is a port of the todo example to PureScript.
To be able to build this, first install purescript and bundling tools:
```
$ npm install -g purescript pulp psc-package parcel-bundler inline-assets
```
Next, install the dependencies:
```
$ psc-package update
```
Now build the frontend and bundle it into `dist/bundle.html`:
```
$ npm run prod
```
Finally use cargo to build the rust executable, which includes `bundle.html` using `include_str!()`.

## elm-counter

(This assumes you're using Elm 0.19.0)

```
$ npm install -g elm
$ cd elm-counter
$ elm make --optimize src/Main.elm
$ cargo run --example elm-counter
```

## actix

Uses [rust-embed](https://github.com/pyros2097/rust-embed) and [actix-web](https://github.com/actix/actix-web) to embed files directly in binary and serve them to web-view.

Unfortunately if you run this with the EdgeHTML backend (`edge` feature) it won't work by default due to webview sandbox restrictions.

In order for this to run on EdgeHTML, you need to run `CheckNetIsolation.exe LoopbackExempt -a -n="Microsoft.Win32WebViewHost_cw5n1h2txyewy"` from your administrator command prompt only once and everything works.

You can make this step for example as a part of your apps installer.

## todo-yew

Based of the code of the actix example (see above) this bundles/serves the yew [todo example](https://github.com/yewstack/yew/tree/master/examples/todomvc) app. That makes it the most `rust`y example and still only has a ~4mb binary size (90% of which is actix actually, see this example repo using hyper to reduce it to 2mb: https://github.com/Extrawurst/rust-webview-todomvc-yew).

Find the build instructions for the todomvc wasm source in `example/todo-yew/Makefile`.

## todo-elm

(This assumes you're using Elm 0.19.0).  
This example is functionally equivalent to `todo` and `todo-purescript` examples, but implemented in Elm.  
It showcases how to communicate from Elm to Rust and back through Elm's ports.  
You can run this example as is with `cargo run --example todo-elm`.  

If you want to edit the example's sources, you will first need to install Elm as described [here](https://guide.elm-lang.org/install/elm.html).  
Then run:  
```
elm make --optimize --output=elm.js src/Main.elm
cargo run --example todo-elm
```
The `--output=elm.js` parameter is very important, otherwise `elm make` would output `index.html`.
We include `elm.js` and js glue code (for Elm's ports) in `todo-elm.rs`, so we cannot use `index.html`.

---

Note: For some reason (at least on Windows), if I try to `cargo run` the examples directly, they don't show the window, but it works with `cargo build --example <name> && target\debug\examples\<name>`
