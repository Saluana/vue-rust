<script setup>
defineEmits(['go'])
</script>
<template>
  <button id="go" @click="$emit('go', 'inner')">go</button>
</template>
