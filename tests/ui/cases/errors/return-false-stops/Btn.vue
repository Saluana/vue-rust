<script setup>
defineProps({ log: null })
function explode() {
  throw new Error('click')
}
</script>
<template>
  <button id="b" @click="explode">b</button>
</template>
