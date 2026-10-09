<script setup>
const { msg = 'fallback', count = 0 } = defineProps({ msg: String, count: Number })
</script>

<template>
  <p>{{ msg }} {{ count }}</p>
</template>
