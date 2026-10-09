<script setup>
import { getCurrentInstance, inject } from 'vue'
const props = defineProps({ log: Array })
const own = getCurrentInstance()
const again = getCurrentInstance()
const owner = inject('owner', null)
props.log.push(['child', own !== null, own === again, owner !== null, owner !== own].join(' '))
</script>
<template>
  <p id="child">child</p>
</template>
