<script setup>
const label = 'scoped'
</script>

<template>
  <p class="s">{{ label }}</p>
</template>

<style scoped>
.s { color: red; }
</style>
