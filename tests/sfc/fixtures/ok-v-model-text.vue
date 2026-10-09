<script setup>
import { ref } from 'vue'

const text = ref('start')
</script>

<template>
  <div>
    <input type="text" v-model="text">
    <p>{{ text }}</p>
  </div>
</template>
