<script setup>
import { ref } from 'vue'
const v = ref(20)
const n = ref(0)
</script>
<template>
  <div>
    <input id="r" type="range" min="0" max="10" v-model="v">
    <input id="s" type="range" :min="0" :max="10" v-model.number="n">
    <p id="out">{{ v }}|{{ n }}</p>
  </div>
</template>
