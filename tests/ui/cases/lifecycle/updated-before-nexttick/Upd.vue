<script setup>
import { ref, nextTick, onUpdated } from 'vue'
const props = defineProps({ log: null })
const n = ref(0)
onUpdated(() => props.log('updated ' + n.value))
function bump() {
  n.value++
  props.log('bumped')
  nextTick(() => props.log('nextTick'))
}
</script>
<template>
  <button id="b" @click="bump">{{ n }}</button>
</template>
