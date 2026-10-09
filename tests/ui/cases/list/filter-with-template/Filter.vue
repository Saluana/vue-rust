<script setup>
defineProps({ items: Array })
</script>
<template>
  <ul>
    <template v-for="x in items" :key="x.id">
      <li v-if="x.on">{{ x.id }}</li>
    </template>
  </ul>
</template>
