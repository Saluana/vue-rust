<script setup>
import { ref, watch } from 'vue'
const props = defineProps({ src: null, log: null })
watch(props.src, (v) => props.log('sync ' + v), { flush: 'sync' })
</script>
<template>
  <p>sync</p>
</template>
