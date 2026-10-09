<script setup>
const props = defineProps({ log: null })
const vFocus = {
  mounted(el) {
    el.focus()
  },
}
</script>
<template>
  <input v-focus @focus="props.log('focused')">
</template>
