<script setup>
import { ref } from 'vue'

const on = ref(true)
</script>

<template>
  <div>
    <button @click="on = !on">toggle</button>
    <span v-if="on">yes</span>
    <span v-else>no</span>
    <em v-if="on">e</em>
  </div>
</template>
