<script setup>
import { provide, ref } from 'vue'
import Leaf from './Leaf.vue'
const count = ref(1)
provide('count', count)
</script>
<template>
  <div><p id="out">{{ count }}</p><Leaf /></div>
</template>
