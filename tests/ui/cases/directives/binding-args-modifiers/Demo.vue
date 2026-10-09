<script setup>
import { ref } from 'vue'
const props = defineProps({ log: null })
const v = ref('value')
const vDemo = {
  mounted(el, b) {
    props.log(b.arg + ' ' + Object.keys(b.modifiers).join('.') + ' ' + b.value)
  },
}
</script>
<template>
  <p v-demo:arg.mod.other="v">x</p>
</template>
