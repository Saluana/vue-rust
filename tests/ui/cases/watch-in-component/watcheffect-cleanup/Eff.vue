<script setup>
import { watchEffect } from 'vue'
const props = defineProps({ n: Number, log: null })
watchEffect((onCleanup) => {
  const v = props.n
  props.log('run ' + v)
  onCleanup(() => props.log('cleanup ' + v))
})
</script>
<template>
  <p>{{ n }}</p>
</template>
