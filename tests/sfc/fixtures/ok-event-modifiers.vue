<script setup>
import { ref } from 'vue'

const hits = ref(0)
</script>

<template>
  <div @click="hits++">
    <button @click.stop="hits += 10">{{ hits }}</button>
  </div>
</template>
