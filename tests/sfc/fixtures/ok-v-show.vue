<script setup>
import { ref } from 'vue'

const visible = ref(true)
</script>

<template>
  <div>
    <button @click="visible = !visible">toggle</button>
    <p v-show="visible">shown</p>
  </div>
</template>
