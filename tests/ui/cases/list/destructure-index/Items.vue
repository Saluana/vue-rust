<script setup>
defineProps({ items: Array })
</script>
<template>
  <ol><li v-for="({ id, tag }, i) in items" :key="id">{{ i }}:{{ tag }}#{{ id }}</li></ol>
</template>
