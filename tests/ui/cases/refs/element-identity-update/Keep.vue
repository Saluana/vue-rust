<script setup>
import { ref } from 'vue'
const props = defineProps({ n: Number, log: null })
const el = ref(null)
const first = ref(null)
</script>
<template>
  <p ref="el">{{ n }}</p>
</template>
