<script setup>
defineEmits(['b'])
</script>
<template>
  <button id="m" @click.left="$emit('b', 'left')" @click.middle="$emit('b', 'middle')" @click.right="$emit('b', 'right')">m</button>
</template>
