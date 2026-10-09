<script setup>
import { ref } from 'vue'
const n = ref(0)
</script>
<template>
  <button id="c" @click="n++">{{ n }}</button>
</template>
