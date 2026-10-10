<script setup>
import { computed, ref } from 'vue'

// A writable computed, derived from count. Its setter throws: a restore that assigned it would fail the restart.
const count = ref(0)
const doubled = computed({
  get: () => count.value * 2,
  set: () => { throw new Error('the setter must not run on restore') },
})
</script>

<template>
  <h1>Computed</h1>
  <p id="count">Count: {{ count }}</p>
  <p id="doubled">Doubled: {{ doubled }}</p>
  <button id="increment" @click="count++">Increment</button>
</template>
