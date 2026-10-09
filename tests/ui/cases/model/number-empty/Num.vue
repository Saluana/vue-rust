<script setup>
import { ref } from 'vue'
const n = ref('')
</script>
<template>
  <div>
    <input id="n" v-model.number="n">
    <p id="out">{{ JSON.stringify(n) }}</p>
  </div>
</template>
