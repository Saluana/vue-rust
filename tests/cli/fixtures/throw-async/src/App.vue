<script setup>
import { ref } from 'vue'

const status = ref('idle')

async function load() {
  status.value = 'loading'
  await Promise.resolve()
  throw new Error('load failed')
}
</script>

<template>
  <button id="go" @click="load">Go</button>
  <p id="status">{{ status }}</p>
</template>
