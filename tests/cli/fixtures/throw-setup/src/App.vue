<script setup>
import { ref } from 'vue'

const count = ref(0)
throw new Error('setup failed')
</script>

<template>
  <p id="count">Count: {{ count }}</p>
</template>
