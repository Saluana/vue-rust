<script setup>
defineEmits(['save'])
</script>
<template>
  <button id="b" @click="$emit('save', 1, 'two', { three: 3 })">b</button>
</template>
