<script setup>
import { ref, onMounted } from 'vue'
import Child from './Child.vue'
const props = defineProps({ log: null })
const c = ref(null)
onMounted(() => {
  c.value.bump()
  c.value.bump()
  props.log('count ' + c.value.count)
})
</script>
<template>
  <Child ref="c" />
</template>
