<script setup>
const emit = defineEmits(['ready'])
emit('ready', 'during-setup')
</script>
<template>
  <p id="p">set</p>
</template>
