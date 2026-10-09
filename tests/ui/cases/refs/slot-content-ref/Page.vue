<script setup>
import { ref, onMounted } from 'vue'
import Shell from './Shell.vue'
const props = defineProps({ log: null })
const inner = ref(null)
onMounted(() => props.log('inner ' + inner.value.id))
</script>
<template>
  <Shell><span id="in" ref="inner">slotted</span></Shell>
</template>
