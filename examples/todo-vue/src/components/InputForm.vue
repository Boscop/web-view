<template>
  <form class="text-input-wrapper" @submit.prevent="submit">
    <input id="task-name-input" class="text-input" type="text" autofocus="true" v-model="text" />
  </form>
</template>

<script>
import { addTask } from "../rpc";

export default {
  data: function() {
    return {
      text: ""
    };
  },
  methods: {
    submit: function(e) {
      addTask(this.text);
      this.text = "";
    }
  }
};
</script>