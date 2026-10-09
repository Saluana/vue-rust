<script setup>
import { ref } from 'vue'

const n = ref(2)
</script>

<template>
  <div>
    <input type="range" v-model="n" min="0" max="5">
    <input v-model.trim="n">
    <p>{{ n }}</p>
  </div>
</template>
