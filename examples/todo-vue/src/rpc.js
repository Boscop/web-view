function invoke(arg) {
    window.external.invoke(JSON.stringify(arg));
}
function init() {
    invoke({ cmd: 'init' });
}
function log() {
    var s = '';
    for (var i = 0; i < arguments.length; i++) {
        if (i != 0) {
            s = s + ' ';
        }
        s = s + JSON.stringify(arguments[i]);
    }
    invoke({ cmd: 'log', text: s });
}
function addTask(name) {
    invoke({ cmd: 'addTask', name: name });
}
function clearDoneTasks() {
    invoke({ cmd: 'clearDoneTasks' });
}
function markTask(index, done) {
    invoke({ cmd: 'markTask', index: index, done: done });
}
function render(items) {
    app.items = items
}

export { invoke, init, log, addTask, clearDoneTasks, markTask, render };