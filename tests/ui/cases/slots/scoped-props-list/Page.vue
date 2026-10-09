<script setup>
import List from './List.vue'
defineProps({ items: Array })
</script>
<template>
  <List :items="items">
    <template #default="{ item, index }">{{ index }}:{{ item.toUpperCase() }}</template>
  </List>
</template>
