<script setup>
import { ref, onMounted } from 'vue'
const props = defineProps({ log: null })
const root = ref(null)
props.log('setup sees ' + (root.value === null))
onMounted(() => props.log('mounted sees ' + root.value.tagName))
</script>
<template>
  <div ref="root">box</div>
</template>
