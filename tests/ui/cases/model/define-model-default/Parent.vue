<script setup>
import { ref } from 'vue'
import Def from './Def.vue'
const text = ref('start')
</script>
<template>
  <div>
    <Def v-model="text" />
    <p id="out">{{ text }}</p>
  </div>
</template>
