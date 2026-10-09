<script setup>
defineProps({
  id: { type: String, required: true },
  size: { validator: (v) => ['s', 'm'].includes(v), default: 's' },
})
</script>
<template>
  <p id="p">{{ id }}-{{ size }}</p>
</template>
