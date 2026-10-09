<script setup>
import { provide } from 'vue'
provide('k', 'mid')
</script>
<template>
  <div><slot /></div>
</template>
