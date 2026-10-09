<script setup>
import { onBeforeMount, onMounted } from 'vue'
const props = defineProps({ name: String, log: null })
onBeforeMount(() => props.log(props.name + ' beforeMount'))
onMounted(() => props.log(props.name + ' mounted'))
</script>
<template>
  <p>{{ name }}</p>
</template>
