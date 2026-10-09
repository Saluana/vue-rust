<script setup>
import Inner from './Inner.vue'
const emit = defineEmits(['seen'])
function onGo(v) {
  emit('seen', 'from-' + v)
}
</script>
<template>
  <div id="o" @click="$emit('seen', 'outer-dom')">
    <Inner @go="onGo" />
  </div>
</template>
