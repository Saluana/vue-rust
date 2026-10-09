<script setup>
import { ref } from 'vue'
import Named from './Named.vue'
const t = ref('a')
</script>
<template>
  <div>
    <Named v-model:title="t" />
    <p id="out">{{ t }}</p>
  </div>
</template>
