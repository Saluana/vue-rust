<script setup>
import { onBeforeMount, onMounted, onBeforeUpdate, onUpdated, onBeforeUnmount, onUnmounted } from 'vue'
const props = defineProps({ log: null, n: Number })
onBeforeMount(() => props.log('child beforeMount'))
onMounted(() => props.log('child mounted'))
onBeforeUpdate(() => props.log('child beforeUpdate'))
onUpdated(() => props.log('child updated'))
onBeforeUnmount(() => props.log('child beforeUnmount'))
onUnmounted(() => props.log('child unmounted'))
</script>
<template>
  <p id="c">{{ n }}</p>
</template>
