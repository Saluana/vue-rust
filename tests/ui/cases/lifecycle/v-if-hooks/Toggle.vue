<script setup>
import Child from './Child.vue'
defineProps({ show: Boolean, log: null })
</script>
<template>
  <div><Child v-if="show" :log="log" :n="7" /></div>
</template>
