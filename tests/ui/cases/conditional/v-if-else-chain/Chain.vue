<script setup>
defineProps({ mode: String })
</script>
<template>
  <div>
    <p v-if="mode === 'a'">A</p>
    <p v-else-if="mode === 'b'">B</p>
    <span v-else>other</span>
    <i v-if="false">never</i>
  </div>
</template>
