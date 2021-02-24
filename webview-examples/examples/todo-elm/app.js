var app = Elm.Main.init({ node: document.getElementById("elm") });
app.ports.toRust.subscribe(function(data) {
    window.external.invoke(JSON.stringify(data));
});