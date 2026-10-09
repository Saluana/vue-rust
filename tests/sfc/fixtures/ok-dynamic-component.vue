<script setup>
import { defineComponent, shallowRef } from 'vue'

const First = defineComponent({ render() { return 'first' } })
const Second = defineComponent({ render() { return 'second' } })
const current = shallowRef(First)
</script>

<template>
  <div>
    <button @click="current = current === First ? Second : First">swap</button>
    <component :is="current" />
  </div>
</template>
