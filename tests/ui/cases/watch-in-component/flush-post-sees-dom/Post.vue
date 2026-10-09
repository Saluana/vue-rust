<script setup>
import { ref, watch } from 'vue'
const props = defineProps({ n: Number, log: null })
const root = ref(null)
watch(() => props.n, () => props.log('pre sees ' + root.value.textContent), { flush: 'pre' })
watch(() => props.n, () => props.log('post sees ' + root.value.textContent), { flush: 'post' })
</script>
<template>
  <p ref="root">{{ n }}</p>
</template>
