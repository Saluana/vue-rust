<script setup>
defineProps({ bad: Boolean })
function boom() {
  throw new Error('bad render')
}
</script>
<template>
  <p v-if="bad">{{ boom() }}</p>
  <p v-else id="ok">fine</p>
</template>
