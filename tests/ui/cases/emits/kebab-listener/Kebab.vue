<script setup>
defineEmits(['someEvent'])
</script>
<template>
  <button id="k" @click="$emit('someEvent', 'x')">k</button>
</template>
