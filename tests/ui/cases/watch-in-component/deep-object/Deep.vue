<script setup>
import { watch } from 'vue'
const props = defineProps({ state: null, log: null })
watch(() => props.state, () => props.log('deep change'), { deep: true })
</script>
<template>
  <p>{{ state.k }}</p>
</template>
