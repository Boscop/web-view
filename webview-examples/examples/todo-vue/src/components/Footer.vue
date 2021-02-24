<template>
  <div class="footer">
    <div class="btn-clear-tasks" @click="clearTasks">Delete completed</div>
  </div>
</template>

<script>
import { clearDoneTasks } from "../rpc";

export default {
  methods: {
    clearTasks: function() {
      clearDoneTasks();
    }
  }
};
</script>

<style scoped>
.footer {
	position: fixed;
	left: 0;
	bottom: 0;
	right: 0;
	background-color: #ffffff;
	color: #9c27b0;
}

.btn-clear-tasks {
	width: 100%;
	text-align: center;
	font-size: 18px;
	height: 2.5em;
	line-height: 2.5em;
	text-transform: uppercase;
	cursor: pointer;
}
</style>