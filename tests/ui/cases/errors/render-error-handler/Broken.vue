<script setup>
function boom() {
  throw new Error('render failed')
}
</script>
<template>
  <p id="x">{{ boom() }}</p>
</template>
