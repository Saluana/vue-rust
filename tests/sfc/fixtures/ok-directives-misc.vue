<script setup>
import { ref } from 'vue'

const n = ref(1)
const attrs = { title: 'spread' }
</script>

<template>
  <div>
    <p v-once>{{ n }}</p>
    <p v-memo="[n]">{{ n }}</p>
    <p v-pre>{{ n }}</p>
    <p v-cloak>{{ n }}</p>
    <p v-text="n"></p>
    <p v-bind="attrs">spread</p>
  </div>
</template>
