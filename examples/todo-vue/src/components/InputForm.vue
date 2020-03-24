<template>
  <form class="text-input-wrapper" @submit.prevent="submit">
    <input id="task-name-input" class="text-input" type="text" autofocus="true" v-on:input="input" />
  </form>
</template>

<script>
import { addTask } from "../rpc";

export default {
  data: function() {
    return {
      description: ""
    };
  },
  methods: {
    submit: function(e) {
      addtask(this.description);
      this.description = "";
      e.target.reset();
    },
    input: function(e) {
      this.description = e.target.value;
    }
  }
};
</script>