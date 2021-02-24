<template>
  <div class="container">
    <input-form></input-form>
    <task-list :tasks="tasks"></task-list>
    <app-fotter></app-fotter>
  </div>
</template>

<script>
import InputForm from "./components/InputForm.vue";
import TaskList from "./components/TaskList.vue";
import AppFotter from "./components/Footer.vue";

export default {
  props: {
    tasks: {
      type: Array,
      required: true
    }
  },
  components: {
    InputForm,
    TaskList,
    AppFotter
  }
};
</script>

<style scoped>
.container {
	width: 100%;
	height: 100%;
	background-color: #9c27b0;
}

</style>

<style>
* {
	margin: 0;
	padding: 0;
	box-sizing: border-box;
	font-size: 28px;
	font-family: sans-serif;
}

html, body {
	height: 100%;
	overflow: none;
}

.ie-upgrade-container {
	width: 100%;
	height: 100%;
	font-family: Arial, sans-serif;
	font-size: 32px;
	color: #ffffff;
	background-color: #1ebbee;
	padding: 3em 1em 1em 1em;
}
.ie-upgrade-link {
	margin: 2em 0em;
	padding: 0 1em;
	color: #1ebbee;
	background-color: #ffffff;
	font-weight: bold;
	text-align: center;
	display: block;
	width: 100%;
	height: 2em;
	line-height: 2em;
	text-transform: uppercase;
}
</style>