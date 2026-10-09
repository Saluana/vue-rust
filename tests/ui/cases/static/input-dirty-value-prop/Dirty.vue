<script setup>
import { ref } from 'vue'
defineProps({ on: Boolean })
const t = ref('start')
</script>
<template>
  <div>
    <input id="i" :value="t">
    <input id="j" value="attr-default">
    <input id="k" type="checkbox" :checked="on">
  </div>
</template>
