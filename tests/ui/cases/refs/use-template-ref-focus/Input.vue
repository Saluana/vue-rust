<script setup>
import { onMounted, useTemplateRef } from 'vue'
const props = defineProps({ log: null })
const field = useTemplateRef('field')
onMounted(() => field.value.focus())
</script>
<template>
  <input ref="field" @focus="props.log('focused')">
</template>
