<script setup>
import { defineAsyncComponent } from 'vue'
const props = defineProps({ loader: null })
const Lazy = defineAsyncComponent(props.loader)
</script>
<template>
  <div><Lazy /></div>
</template>
