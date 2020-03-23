'use strict';

Vue.component('input-form', {
  data() {
    return {
      description: ""
    }
  },
  methods: {
    submit(e) {
      e.preventDefault();
      e.stopImmediatePropagation();
      rpc.addTask(this.description);
      this.description = "";
      e.target.reset();
    },
    input(e) {
      this.description = e.target.value
    }
  },
  render(h) {
    return (
      h('form', { attrs: { class: 'text-input-wrapper' }, on: { submit: this.submit } }, [
        h('input', {
          attrs: {
            id: 'task-name-input',
            class: 'text-input',
            type: 'text',
            autofocus: true
          },
          on: { input: this.input }
        })
      ])
    )
  }
})

function markTask(i, done) { return () => rpc.markTask(i, !done) }
Vue.component('task-list', {
  functional: true,
  props: {
    items: {
      type: Array,
      required: true
    }
  },
  render(h, ctx) {
    let items = ctx.props.items  // alias
    let taskItems = [];
    for (var i = 0; i < items.length; i++) {
      let checked = (items[i].done ? 'checked' : 'unchecked');
      taskItems.push(h('div', {
        attrs: { class: 'task-item ' + checked },
        on: { click: markTask(i, items[i].done) }
      }, items[i].name))
    };

    return (
      h('div', { attrs: { class: 'task-list' } }, taskItems)
    )
  }
})

function clearTasks() { rpc.clearDoneTasks(); }
Vue.component('app-footer', {
  functional: true,
  render(h, ctx) {
    return (
      h('div', { attrs: { class: 'footer' } }, [
        h('div', {
          attrs: { class: 'btn-clear-tasks' },
          on: { click: clearTasks }
        },
          'Delete completed')
      ])
    )
  }
})

let app = new Vue({
  el: "#app",
  data() {
    return {
      items: []
    }
  },
  render(h) {

    return h('div', { attrs: { class: 'container' } }, [
      h('input-form'),
      h('task-list', { attrs: { items: this.items } }),
      h('app-footer')
    ])
  }
})

let rpc = {
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
    app.items = items
  },
};

window.onload = function () { rpc.init(); };;