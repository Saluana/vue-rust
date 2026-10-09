<script setup>
import { onUpdated } from 'vue'
import Own from './Own.vue'
const props = defineProps({ log: null })
onUpdated(() => props.log('host updated'))
</script>
<template>
  <div><Own :log="log" /></div>
</template>
