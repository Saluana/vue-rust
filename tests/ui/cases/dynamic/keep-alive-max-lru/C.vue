<script setup>
import { ref, onMounted, onActivated, onDeactivated, onUnmounted } from 'vue'
const props = defineProps({ log: null })
const n = ref(0)
function bump() {
  n.value++
}
onMounted(() => props.log('C mounted'))
onActivated(() => props.log('C activated'))
onDeactivated(() => props.log('C deactivated'))
onUnmounted(() => props.log('C unmounted'))
</script>
<template>
  <button id="btn" @click="bump">C {{ n }}</button>
</template>
