<script setup>
defineProps({ items: Array, registry: Object })
</script>
<template>
  <ul>
    <li v-for="it in items" :key="it.id" :data-id="it.id" :ref="(el) => { if (el) registry[it.id] = el }">{{ it.label }}</li>
  </ul>
</template>
