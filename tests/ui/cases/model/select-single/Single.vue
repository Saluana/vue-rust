<script setup>
import { ref } from 'vue'
const v = ref('a')
</script>
<template>
  <div>
    <select id="s" v-model="v">
      <option value="a">A</option>
      <option value="b">B</option>
    </select>
    <p id="out">{{ v }}</p>
  </div>
</template>
