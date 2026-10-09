<script setup>
import Child from './Child.vue'
defineProps({ show: Boolean })
</script>
<template>
  <div>
    <Child v-show="show" />
  </div>
</template>
