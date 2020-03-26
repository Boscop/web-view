<template>
  <form class="text-input-wrapper" @submit.prevent="submit">
    <input class="text-input" type="text" autofocus="true" v-model="text" />
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

<style scoped>
.text-input-wrapper {
  padding: 0.5em;
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
}
.text-input {
  width: 100%;
  line-height: 1.5em;
  padding: 0 0.2em;
  height: 1.5em;
  outline: none;
  border: none;
  color: #4a148c;
  background-color: rgba(255, 255, 255, 0.87);
}
.text-input:focus {
  background-color: #ffffff;
}
</style>