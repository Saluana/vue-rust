<script setup>
import { onErrorCaptured } from 'vue'
import Btn from './Btn.vue'
const props = defineProps({ log: null })
onErrorCaptured((err, instance, info) => {
  props.log('parent saw ' + err.message + ' ' + info)
  return false
})
</script>
<template>
  <div><Btn :log="log" /></div>
</template>
