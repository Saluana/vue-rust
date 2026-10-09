<script setup>
import { inject } from 'vue'

const theme = inject('theme', 'light')
</script>

<template>
  <p>{{ theme }}</p>
</template>
