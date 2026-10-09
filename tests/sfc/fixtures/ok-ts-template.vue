<script setup lang="ts">
const n = 2 as number
</script>

<template>
  <p :title="n as number">{{ (n as number) + 1 }}</p>
</template>
