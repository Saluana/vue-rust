<script setup>
import { watch } from 'vue'
const props = defineProps({ src: null, log: null })
watch(props.src, (v) => props.log('watch ' + v))
</script>
<template>
  <p>watching</p>
</template>
