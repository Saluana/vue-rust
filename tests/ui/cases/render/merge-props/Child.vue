<script setup>
import { mergeProps } from 'vue'
const props = defineProps({ log: Array })
const attrs = mergeProps(
  { class: 'a', style: 'color: red', onClick: () => props.log.push('first') },
  { class: ['b', { c: true }], style: { margin: '0px' }, onClick: () => props.log.push('second'), title: 'T' },
)
</script>
<template>
  <button v-bind="attrs">go</button>
</template>
