<script setup>
import { ref } from 'vue'
const s = ref('')
</script>
<template>
  <div>
    <textarea id="t" v-model="s"></textarea>
    <p id="out">{{ s.length }}</p>
  </div>
</template>
