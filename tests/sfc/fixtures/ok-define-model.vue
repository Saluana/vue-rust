<script setup>
const model = defineModel({ default: 'a' })
</script>

<template>
  <input type="text" :value="model" @input="model = $event.target.value">
</template>
