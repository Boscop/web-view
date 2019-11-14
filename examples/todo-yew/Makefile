clean: 
	rm static/todomvc.js
	rm static/todomvc.wasm

build-todomvc:
	cd todomvc && cargo web build --release
	cp todomvc/target/wasm32-unknown-unknown/release/todomvc.js static/
	cp todomvc/target/wasm32-unknown-unknown/release/todomvc.wasm static/