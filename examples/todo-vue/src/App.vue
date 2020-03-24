<template>
  <div>
    <input-form></input-form>
    <task-list :items="items"></task-list>
    <app-fotter></app-fotter>
  </div>
</template>

<script>
import InputForm from "./components/InputForm.vue";
import TaskList from "./components/TaskList.vue";
import AppFotter from "./components/Footer.vue";

export default {
  props: {
    items: {
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