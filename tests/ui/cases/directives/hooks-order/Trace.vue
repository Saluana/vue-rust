<script setup>
import { ref } from 'vue'
const props = defineProps({ n: Number, log: null })
const vTrace = {
  created(el, b) { props.log('created ' + b.value) },
  beforeMount(el, b) { props.log('beforeMount ' + b.value) },
  mounted(el, b) { props.log('mounted ' + b.value) },
  beforeUpdate(el, b) { props.log('beforeUpdate ' + b.value + ' old ' + b.oldValue) },
  updated(el, b) { props.log('updated ' + b.value) },
  beforeUnmount(el, b) { props.log('beforeUnmount ' + b.value) },
  unmounted(el, b) { props.log('unmounted ' + b.value) },
}
</script>
<template>
  <p v-trace="n">{{ n }}</p>
</template>
