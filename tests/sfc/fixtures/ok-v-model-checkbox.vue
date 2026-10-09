<script setup>
import { ref } from 'vue'

const agree = ref(false)
</script>

<template>
  <div>
    <input type="checkbox" v-model="agree">
    <p>{{ agree ? 'yes' : 'no' }}</p>
  </div>
</template>
