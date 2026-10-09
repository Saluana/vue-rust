<script setup lang="ts" generic="T extends string">
defineProps<{ items: T[] }>()
</script>

<template>
  <ul><li v-for="item in items" :key="item">{{ item }}</li></ul>
</template>
