<script setup>
let count = 0
function go() {
  count++
  if (count === 1) throw new Error('first click')
}
</script>
<template>
  <button id="b" @click="go">b</button>
</template>
