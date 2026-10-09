<script setup>
defineProps({ on: Boolean, extra: String })
const base = { color: 'red' }
</script>
<template>
  <div class="a" :class="['b', { c: on }, extra]" :style="[base, { margin: '0px' }]" data-k="v">
    <span :class="{ on }" style="width: 10px" :style="{ height: on ? '5px' : '' }">s</span>
  </div>
</template>
