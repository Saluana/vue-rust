<script setup>
defineProps({ items: Array, reg: Object })
</script>
<template>
  <ul>
    <li v-for="it in items" :key="it" :ref="(el) => { if (el) reg[it] = el }">{{ it }}</li>
  </ul>
</template>
