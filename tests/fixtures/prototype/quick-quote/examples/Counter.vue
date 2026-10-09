<script setup lang="ts">
import { ref, computed } from 'vue'
const count = ref(0)
const name = ref('Brendon')
const doubled = computed(() => count.value * 2)
const greeting = computed(() => 'Hello, ' + name.value)
function increment() { count.value++ }
function reset() { count.value = 0 }
</script>

<template>
  <main>
    <h1>{{ greeting }}</h1>
    <p>Vue syntax. Rust-rendered UI.</p>
    <input v-model="name" />
    <p>Count: {{ count }} / Double: {{ doubled }}</p>
    <button @click="increment">Add one</button>
    <button @click="reset" :disabled="count === 0">Reset</button>
    <p v-if="count > 5">You clicked more than five times.</p>
  </main>
</template>
