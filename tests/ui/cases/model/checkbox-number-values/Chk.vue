<script setup>
import { ref } from 'vue'
const n = ref(0)
</script>
<template>
  <div>
    <input id="c" type="checkbox" v-model="n" :true-value="1" :false-value="0">
    <p id="out">{{ n === 1 }}</p>
  </div>
</template>
