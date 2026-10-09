<script setup>
import { ref, onBeforeUpdate, onUpdated } from 'vue'
const props = defineProps({ log: null })
const n = ref(0)
onBeforeUpdate(() => props.log('own beforeUpdate'))
onUpdated(() => props.log('own updated'))
function bump() {
  n.value++
}
</script>
<template>
  <button id="b" @click="bump">{{ n }}</button>
</template>
