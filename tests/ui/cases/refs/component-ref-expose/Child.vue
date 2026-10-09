<script setup>
import { ref } from 'vue'
const count = ref(0)
function bump() {
  count.value++
}
defineExpose({ count, bump })
</script>
<template>
  <p>child</p>
</template>
