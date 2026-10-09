<script setup>
import { provide } from 'vue'
provide('x', 'v')
</script>
<template>
  <div><slot /></div>
</template>
