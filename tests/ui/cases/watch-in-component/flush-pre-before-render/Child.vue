<script setup>
import { watch, onUpdated } from 'vue'
const props = defineProps({ n: Number, log: null })
watch(() => props.n, () => props.log('watch'))
onUpdated(() => props.log('updated'))
</script>
<template>
  <p>{{ n }}</p>
</template>
