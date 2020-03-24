<template>
  <div class="task-list">
    <div
      :class="isDone(item)"
      v-for="(item, index) in items"
      :key="`item.name-${index}`"
      @click="_markTask(index, item.done)"
    >{{item.name}}</div>
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
    isDone: function(item) {
      let checked = item.done ? "checked" : "unchecked";
      return "task-item " + checked;
    },
    _markTask: function(i, done) {
      markTask(i, !done);
    }
  }
};
</script>