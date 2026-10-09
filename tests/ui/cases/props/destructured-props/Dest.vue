<script setup>
const { size = 'md', label } = defineProps({ size: String, label: String })
</script>
<template>
  <p id="p">{{ size }}|{{ label }}</p>
</template>
