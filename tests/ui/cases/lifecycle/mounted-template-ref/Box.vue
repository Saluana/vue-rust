<script setup>
import { ref, onMounted } from 'vue'
const props = defineProps({ log: null })
const root = ref(null)
onMounted(() => props.log('mounted ' + root.value.id))
</script>
<template>
  <div id="box" ref="root">box</div>
</template>
