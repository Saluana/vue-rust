<script setup>
import { onErrorCaptured } from 'vue'
import Bad from './Bad.vue'
const props = defineProps({ log: null })
onErrorCaptured((err, instance, info) => {
  props.log('captured ' + err.message + ' ' + info)
})
</script>
<template>
  <div><Bad /></div>
</template>
