<script setup>
import { ref } from 'vue'
const list = ref([])
</script>
<template>
  <div>
    <select id="s" multiple v-model="list">
      <option value="a">A</option>
      <option value="b">B</option>
      <option value="c">C</option>
    </select>
    <p id="out">{{ list.join(',') }}</p>
  </div>
</template>
