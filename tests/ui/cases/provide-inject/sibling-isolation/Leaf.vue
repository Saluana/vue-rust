<script setup>
import { inject } from 'vue'
const v = inject('x', 'none')
</script>
<template>
  <p class="leaf">{{ v }}</p>
</template>
