<script setup>
import { provide } from 'vue'
import Mid from './Mid.vue'
provide('k', 'outer')
</script>
<template>
  <div><Mid /></div>
</template>
