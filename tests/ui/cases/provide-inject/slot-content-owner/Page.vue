<script setup>
import { provide } from 'vue'
import Mid from './Mid.vue'
import Leaf from './Leaf.vue'
provide('k', 'page')
</script>
<template>
  <Mid><Leaf /></Mid>
</template>
