<script setup>
import { onBeforeMount, onMounted, onBeforeUpdate, onUpdated, onBeforeUnmount, onUnmounted } from 'vue'
import Child from './Child.vue'
const props = defineProps({ log: null, n: Number })
onBeforeMount(() => props.log('parent beforeMount'))
onMounted(() => props.log('parent mounted'))
onBeforeUpdate(() => props.log('parent beforeUpdate'))
onUpdated(() => props.log('parent updated'))
onBeforeUnmount(() => props.log('parent beforeUnmount'))
onUnmounted(() => props.log('parent unmounted'))
</script>
<template>
  <Child :log="log" :n="n" />
</template>
