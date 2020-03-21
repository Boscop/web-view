'use strict';

var data = {
  description: "",
  items: []
}

function submit(e) {
  e.preventDefault();
  e.stopImmediatePropagation();
  rpc.addTask(data.description);
  data.description = "";
  e.target.reset();
}
function clearTasks() { rpc.clearDoneTasks(); }
function markTask(i) { return function () { rpc.markTask(i, !data.items[i].done); } };

var app = new Vue({
  el: "#app",
  data,
  render: (h) => {
    var taskItems = [];
    for (var i = 0; i < data.items.length; i++) {
      var checked = (data.items[i].done ? 'checked' : 'unchecked');
      taskItems.push(h('div', {
        attrs: { class: 'task-item ' + checked },
        on: { click: markTask(i) }
      }, data.items[i].name))
    };

    return h('div', { attrs: { class: 'container' } }, [
      h('form', { on: { submit: submit } }, [
        h('input', {
          attrs: {
            id: 'task-name-input',
            class: 'text-input',
            type: 'text',
            autofocus: true
          },
          on: {
            input: (e) => {
              data.description = e.target.value
            }
          }
        })]),
      h('div', { attrs: { class: 'task-list' } }, taskItems),
      h('div', { attrs: { class: 'footer' } }, [
        h('div', {
          attrs: { class: 'btn-clear-tasks' },
          on: { click: clearTasks }
        },
          'Delete completed')
      ])
    ])
  }
})

var rpc = {
  invoke: function (arg) { window.external.invoke(JSON.stringify(arg)); },
  init: function () { rpc.invoke({ cmd: 'init' }); },
  log: function () {
    var s = '';
    for (var i = 0; i < arguments.length; i++) {
      if (i != 0) {
        s = s + ' ';
      }
      s = s + JSON.stringify(arguments[i]);
    }
    rpc.invoke({ cmd: 'log', text: s });
  },
  addTask: function (name) { rpc.invoke({ cmd: 'addTask', name: name }); },
  clearDoneTasks: function () { rpc.invoke({ cmd: 'clearDoneTasks' }); },
  markTask: function (index, done) {
    rpc.invoke({ cmd: 'markTask', index: index, done: done });
  },
  render: function (items) {
    data.items = items
  },
};

window.onload = function () { rpc.init(); };