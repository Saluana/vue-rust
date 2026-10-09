<script setup>
defineProps({ items: Array })
</script>
<template>
  <ol><li v-for="(x, i) in items" :key="i">{{ i }}:{{ x }}</li></ol>
</template>
