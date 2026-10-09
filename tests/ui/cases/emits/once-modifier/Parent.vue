<script setup>
import Child from './Child.vue'
defineProps({ log: null })
</script>
<template>
  <div>
    <Child @hit.once="log('once')" />
  </div>
</template>
