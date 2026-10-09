<script setup>
import { ref, onMounted, onActivated, onDeactivated, onUnmounted } from 'vue'
const props = defineProps({ log: null })
const n = ref(0)
function bump() {
  n.value++
}
onMounted(() => props.log('B mounted'))
onActivated(() => props.log('B activated'))
onDeactivated(() => props.log('B deactivated'))
onUnmounted(() => props.log('B unmounted'))
</script>
<template>
  <button id="btn" @click="bump">B {{ n }}</button>
</template>
