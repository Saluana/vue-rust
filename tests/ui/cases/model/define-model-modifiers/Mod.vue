<script setup>
const [model, modifiers] = defineModel({
  set(value) {
    if (modifiers.trim) return value.trim()
    return value
  },
})
</script>
<template>
  <input id="m" v-model="model">
</template>
