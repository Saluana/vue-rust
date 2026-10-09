<script setup>
async function load() {
  await null
  throw new Error('later')
}
</script>
<template>
  <button id="b" @click="load">b</button>
</template>
