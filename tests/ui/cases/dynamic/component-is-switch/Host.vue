<script setup>
defineProps({ current: null })
</script>
<template>
  <div><component :is="current" /></div>
</template>
