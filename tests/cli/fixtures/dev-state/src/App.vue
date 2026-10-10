<script setup lang="ts">
import { ref, reactive } from 'vue'

// State of the dev-loop tests (tests/cli/dev.test.mjs). count and label are refs of plain data, onTick is a closure held
// in a ref (never snapshotted), settings is a reactive object of plain data.
const count = ref(0)
const label = ref('a')
const onTick = ref(() => count.value + 100)
const settings = reactive({ step: 1, title: 'x' })

function bump() {
  count.value += settings.step
}
</script>

<template>
  <main>
    <h1>State</h1>
    <p id="count">Count: {{ count }}</p>
    <p id="label">Label: {{ label }}</p>
    <p id="tick">Tick: {{ onTick() }}</p>
    <p id="title">Title: {{ settings.title }}</p>
    <button id="bump" @click="bump">Bump</button>
  </main>
</template>
