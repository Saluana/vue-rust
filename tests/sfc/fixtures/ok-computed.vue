<script setup>
import { computed, ref } from 'vue'

const base = ref(2)
const doubled = computed(() => base.value * 2)
</script>

<template>
  <div>
    <button @click="base++">{{ base }}</button>
    <span>{{ doubled }}</span>
  </div>
</template>
