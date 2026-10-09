<script setup>
import { inject } from 'vue'
const count = inject('count')
function bump() {
  count.value++
}
</script>
<template>
  <button id="b" @click="bump">+</button>
</template>
