<script setup>
import { ref } from 'vue'
const text = ref('')
</script>
<template>
  <div>
    <input id="c" v-model="text">
    <p id="out">{{ text }}</p>
  </div>
</template>
