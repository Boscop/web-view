<template>
  <div class="footer">
    <div class="btn-clear-tasks" @click="clearTasks">Delete completed</div>
  </div>
</template>

<script>
import { clearDoneTasks } from "./rpc";
export default {
  methods: {
    clearTasks: function() {
      clearDoneTasks();
    }
  }
};
</script>