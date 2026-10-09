<script setup>
defineEmits(['x'])
</script>
<template>
  <button id="x" @click.ctrl.exact="$emit('x', 'ctrl')">x</button>
</template>
