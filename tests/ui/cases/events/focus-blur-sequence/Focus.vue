<script setup>
defineEmits(['ev'])
</script>
<template>
  <div>
    <input id="a" @focus="$emit('ev', 'focus-a')" @blur="$emit('ev', 'blur-a')">
    <input id="b" @focus="$emit('ev', 'focus-b')" @blur="$emit('ev', 'blur-b')">
  </div>
</template>
