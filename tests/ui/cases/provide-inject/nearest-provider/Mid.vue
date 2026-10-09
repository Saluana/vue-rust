<script setup>
import { provide } from 'vue'
import Leaf from './Leaf.vue'
provide('k', 'mid')
</script>
<template>
  <div><Leaf /></div>
</template>
