<script setup>
import { inject } from 'vue'
const v = inject('k', 'none')
</script>
<template>
  <p id="leaf">{{ v }}</p>
</template>
