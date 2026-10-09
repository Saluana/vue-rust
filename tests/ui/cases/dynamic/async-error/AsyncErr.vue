<script setup>
import { defineAsyncComponent } from 'vue'
import ErrComp from './ErrComp.vue'
const props = defineProps({ loader: null })
const Lazy = defineAsyncComponent({ loader: props.loader, errorComponent: ErrComp })
</script>
<template>
  <div><Lazy /></div>
</template>
