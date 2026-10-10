<script setup>
import { getCurrentInstance, provide } from 'vue'
import Child from './Child.vue'
const props = defineProps({ log: Array })
const self = getCurrentInstance()
provide('owner', self)
props.log.push(['parent', self !== null].join(' '))
</script>
<template>
  <Child :log="props.log" />
</template>
