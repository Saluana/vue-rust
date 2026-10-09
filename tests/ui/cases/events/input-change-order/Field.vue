<script setup>
defineEmits(['ev'])
</script>
<template>
  <input id="f" @input="$emit('ev', 'input')" @change="$emit('ev', 'change')">
</template>
