<script setup lang="ts">
import { ref, computed } from 'vue'

const count = ref(0)
const doubled = computed(() => count.value * 2)

function increment() {
  count.value++
}

function reset() {
  count.value = 0
}
</script>

<template>
  <main class="counter">
    <h1>Counter</h1>
    <p id="count">Count: {{ count }}</p>
    <p id="doubled">Double: {{ doubled }}</p>
    <button id="increment" @click="increment">Add one</button>
    <button id="reset" :disabled="count === 0" @click="reset">Reset</button>
    <p id="many" v-if="count > 5">You clicked more than five times.</p>
  </main>
</template>

<style scoped>
.counter {
  font-family: sans-serif;
  max-width: 24rem;
  margin: 2rem auto;
}
button {
  margin-right: 0.5rem;
}
</style>
