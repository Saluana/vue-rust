<script setup>
defineEmits(['save'])
</script>
<template>
  <button id="n" @click="$emit('save')">n</button>
</template>
