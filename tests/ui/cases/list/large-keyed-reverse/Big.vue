<script setup>
defineProps({ rows: Array })
</script>
<template>
  <div>
    <p v-for="r in rows" :key="r">row {{ r }}</p>
  </div>
</template>
