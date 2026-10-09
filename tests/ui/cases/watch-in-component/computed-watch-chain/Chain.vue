<script setup>
import { computed, watch } from 'vue'
const props = defineProps({ n: Number, log: null })
const doubled = computed(() => props.n * 2)
watch(doubled, (v) => props.log('doubled ' + v))
</script>
<template>
  <p id="d">{{ doubled }}</p>
</template>
