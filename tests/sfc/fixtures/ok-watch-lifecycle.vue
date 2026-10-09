<script setup>
import { ref, watch, onMounted } from 'vue'

const count = ref(0)
const status = ref('idle')

watch(count, (n) => {
  status.value = 'changed to ' + n
})

onMounted(() => {
  status.value = 'mounted'
})
</script>

<template>
  <div>
    <button @click="count++">{{ count }}</button>
    <p>{{ status }}</p>
  </div>
</template>
