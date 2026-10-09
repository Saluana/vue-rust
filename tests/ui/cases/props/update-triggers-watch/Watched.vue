<script setup>
import { watch } from 'vue'
const props = defineProps({ n: Number, log: null })
watch(() => props.n, (now, before) => props.log('change', before, now))
</script>
<template>
  <p id="p">{{ n }}</p>
</template>
