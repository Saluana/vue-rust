<script setup>
import { ref } from 'vue'

const color = ref('red')
</script>

<template>
  <div>
    <select v-model="color">
      <option value="red">red</option>
      <option value="blue">blue</option>
    </select>
    <p>{{ color }}</p>
  </div>
</template>
