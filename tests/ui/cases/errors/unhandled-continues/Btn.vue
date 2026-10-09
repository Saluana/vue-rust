<script setup>
import { ref } from 'vue'
const n = ref(0)
function go() {
  n.value++
  if (n.value === 1) throw new Error('unhandled')
}
</script>
<template>
  <button id="b" @click="go">{{ n }}</button>
</template>
