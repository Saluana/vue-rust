<script setup>
import { ref } from 'vue'
const n = ref(1)
</script>
<template>
  <div>
    <select id="s" v-model="n">
      <option :value="1">one</option>
      <option :value="2">two</option>
    </select>
    <p id="out">{{ n === 2 }}</p>
  </div>
</template>
