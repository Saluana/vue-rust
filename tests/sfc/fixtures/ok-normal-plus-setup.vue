<script>
export default {
  name: 'Named',
  inheritAttrs: false,
}
</script>

<script setup>
import { ref } from 'vue'

const n = ref(1)
</script>

<template>
  <button @click="n++">{{ n }}</button>
</template>
