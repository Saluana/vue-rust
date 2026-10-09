<script setup>
defineEmits(['key'])
</script>
<template>
  <div>
    <input id="k" @keydown.enter="$emit('key', 'enter')" @keydown.ctrl.enter.exact="$emit('key', 'ctrl-enter')" @keydown.esc="$emit('key', 'esc')">
  </div>
</template>
