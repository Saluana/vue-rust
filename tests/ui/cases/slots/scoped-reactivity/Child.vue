<script setup>
import { ref } from 'vue'
const value = ref('a')
defineExpose({ value })
</script>
<template>
  <div><slot :v="value" /></div>
</template>
