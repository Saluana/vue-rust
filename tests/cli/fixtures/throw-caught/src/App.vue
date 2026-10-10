<script setup>
import { ref } from 'vue'

// A throw statement that a try/catch in the same component catches. Its error's stack must keep the text that every
// error has: the name, the message and the "at <anonymous>" line, with no source position.
const verdict = ref('not run')
try {
  throw new Error('caught')
} catch (e) {
  verdict.value = e.stack === 'Error: caught\n    at <anonymous>' ? 'unchanged' : 'changed'
}
</script>

<template>
  <p id="verdict">{{ verdict }}</p>
</template>
