import Vue from 'vue';
import App from './App.vue';
import { init } from './rpc';

let app = new Vue({
  el: "#app",
  data: function () {
    return {
      items: []
    }
  },
  render: function (h) {
    return h(App, { attrs: { items: this.items } })
  }
});

// window.onload = function () { init(); };

export { app };