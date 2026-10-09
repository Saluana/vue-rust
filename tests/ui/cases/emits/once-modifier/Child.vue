<script setup>
defineEmits(['hit'])
</script>
<template>
  <button id="b" @click="$emit('hit')">b</button>
</template>
