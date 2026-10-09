<script setup>
defineProps({ items: Array })
</script>
<template>
  <ul>
    <li v-for="(it, i) in items" :key="i"><slot :item="it" :index="i">{{ it }}</slot></li>
  </ul>
</template>
