<script setup>
import { defineAsyncComponent } from 'vue'
import Loading from './Loading.vue'
const props = defineProps({ loader: null })
const Lazy = defineAsyncComponent({ loader: props.loader, loadingComponent: Loading, delay: 50 })
</script>
<template>
  <div><Lazy /></div>
</template>
