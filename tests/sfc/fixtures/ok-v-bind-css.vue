<script setup>
import { ref } from 'vue'

const color = ref('red')
</script>

<template>
  <p class="c" @click="color = 'blue'">tint</p>
</template>

<style>
.c { color: v-bind(color); }
</style>
