<script setup>
import { ref } from 'vue'

const rows = ref(['x', 'y'])

function shuffle() {
  rows.value = [...rows.value].reverse()
}
</script>

<template>
  <div>
    <button @click="shuffle">shuffle</button>
    <p v-for="row in rows" :key="row">{{ row }}</p>
  </div>
</template>
