<script setup>
import Child from './Child.vue'
const props = defineProps({ log: null })
const vTag = {
  mounted(el) {
    props.log(el.tagName + ' ' + el.className)
  },
}
</script>
<template>
  <div>
    <Child v-tag />
  </div>
</template>
