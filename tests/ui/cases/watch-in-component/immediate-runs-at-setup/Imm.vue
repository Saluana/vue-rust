<script setup>
import { watch } from 'vue'
const props = defineProps({ n: Number, log: null })
watch(() => props.n, (v) => props.log('watch ' + v), { immediate: true })
</script>
<template>
  <p>{{ n }}</p>
</template>
