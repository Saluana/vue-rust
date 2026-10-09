<script setup>
import { ref, onMounted } from 'vue'
import Child from './Child.vue'
const props = defineProps({ log: null })
const c = ref(null)
onMounted(() => props.log('visible ' + c.value.visible + ' secret-undefined ' + (c.value.secret === undefined)))
</script>
<template>
  <Child ref="c" />
</template>
