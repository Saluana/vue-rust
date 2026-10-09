<script setup>
import { ref } from 'vue'
const n = ref(0)
</script>
<template><button id="cached" @click="n++">A {{ n }}</button></template>
