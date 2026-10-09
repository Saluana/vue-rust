<script setup>
defineProps({ tag: String })
</script>
<template>
  <div><component :is="tag">hi</component></div>
</template>
