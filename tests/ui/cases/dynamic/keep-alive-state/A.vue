<script setup>
import { ref, onMounted, onActivated, onDeactivated, onUnmounted } from 'vue'
const props = defineProps({ log: null })
const n = ref(0)
function bump() {
  n.value++
}
onMounted(() => props.log('A mounted'))
onActivated(() => props.log('A activated'))
onDeactivated(() => props.log('A deactivated'))
onUnmounted(() => props.log('A unmounted'))
</script>
<template>
  <button id="btn" @click="bump">A {{ n }}</button>
</template>
