<script setup>
import { onBeforeMount, onMounted } from 'vue'
const props = defineProps({ log: null })
props.log('setup')
onBeforeMount(() => props.log('beforeMount'))
onMounted(() => props.log('mounted'))
</script>
<template>
  <p id="p">x</p>
</template>
