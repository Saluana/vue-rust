<script setup>
const emit = defineEmits({ save: (p) => p.ok === true })
</script>
<template>
  <button id="b" @click="emit('save', { ok: false })">b</button>
</template>
