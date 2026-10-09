<script setup>
import { ref } from 'vue'
import Child from './Child.vue'
const count = ref(0)
</script>
<template>
  <div>
    <button id="inc" @click="count++">+</button>
    <Child>
      <template #default="{ v }">{{ v }}-{{ count }}</template>
    </Child>
  </div>
</template>
