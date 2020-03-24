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

<style scoped>
.task-list {
  overflow-y: auto;
  position: fixed;
  top: 2.5em;
  bottom: 1.2em;
  left: 0;
  right: 0;
}
.task-item {
  height: 1.5em;
  color: rgba(255, 255, 255, 0.87);
  padding: 0.5em;
  cursor: pointer;
}
.task-item.checked {
  text-decoration: line-through;
  color: rgba(255, 255, 255, 0.38);
}
</style>