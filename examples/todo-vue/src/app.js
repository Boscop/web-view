import Vue from 'vue';
import App from './App.vue';
import { init } from './rpc';

let vm = new Vue({
  el: "#app",
  data: function () {
    return {
      tasks: []
    }
  },
  render: function (h) {
    return h(App, { attrs: { tasks: this.tasks } })
  }
});

window.onload = function () { init(); };

function fromRust(tasks) {
  vm.tasks = tasks;
}

export { fromRust };