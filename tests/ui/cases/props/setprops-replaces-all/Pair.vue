<script setup>
defineProps({ a: String, b: String })
</script>
<template>
  <p id="p">{{ a }}|{{ b }}</p>
</template>
