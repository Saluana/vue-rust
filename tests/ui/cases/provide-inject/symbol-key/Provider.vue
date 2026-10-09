<script setup>
import { provide } from 'vue'
import Leaf from './Leaf.vue'
const props = defineProps({ k: null })
provide(props.k, 'sym-value')
</script>
<template>
  <div><Leaf :k="k" /></div>
</template>
