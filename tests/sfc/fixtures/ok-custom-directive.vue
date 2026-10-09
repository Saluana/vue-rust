<script setup>
const vFill = {
  mounted(el, binding) {
    el.setAttribute('data-fill', binding.value)
  },
}
</script>

<template>
  <p v-fill="'blue'">x</p>
</template>
