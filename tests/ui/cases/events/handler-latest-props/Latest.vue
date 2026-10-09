<script setup>
const props = defineProps({ label: String })
const emit = defineEmits(['hit'])
</script>
<template>
  <button id="b" @click="emit('hit', props.label)">b</button>
</template>
