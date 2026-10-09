<script setup>
import { ref } from 'vue'
const visible = 'yes'
const secret = ref('hidden')
defineExpose({ visible })
</script>
<template>
  <p>child</p>
</template>
