<script setup>
defineEmits(['log'])
</script>
<template>
  <div id="p" @click.capture="$emit('log', 'parent-capture')" @click="$emit('log', 'parent-bubble')">
    <button id="c" @click="$emit('log', 'child')">c</button>
  </div>
</template>
