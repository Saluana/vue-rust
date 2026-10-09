<script setup>
import { ref } from 'vue'
const pick = ref('b')
</script>
<template>
  <div>
    <select id="s1">
      <option value="a">A</option>
      <option value="b">B</option>
    </select>
    <select id="s2" :value="pick">
      <option value="a">A</option>
      <option value="b" selected>B</option>
    </select>
  </div>
</template>
