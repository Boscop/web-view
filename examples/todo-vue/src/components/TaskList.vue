<template>
  <div class="task-list">
    <div class="task-item" v-for="item in items" :key="item.name" @click="_markTask">{{item.name}}</div>
  </div>
</template>

<script>
import { markTask } from "../rpc";

export default {
  props: {
    items: {
      type: Array,
      required: true
    }
  },
  methods: {
    _markTask: function(i, done) {
      return function() {
        markTask(i, !done);
      };
    }
  }
  //   render: function(h, ctx) {
  //     let items = ctx.props.items; // alias
  //     let taskItems = [];
  //     for (var i = 0; i < items.length; i++) {
  //       let checked = items[i].done ? "checked" : "unchecked";
  //       taskItems.push(
  //         h(
  //           "div",
  //           {
  //             attrs: { class: "task-item " + checked },
  //             on: { click: _markTask(i, items[i].done) }
  //           },
  //           items[i].name
  //         )
  //       );
  //     }
};
</script>