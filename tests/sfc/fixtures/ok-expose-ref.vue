<script setup>
import { ref } from 'vue'

const box = ref(null)
const log = ref('none')

function hello(name) {
  log.value = 'hello ' + name
}

defineExpose({ hello })
</script>

<template>
  <div>
    <p ref="box">{{ log }}</p>
  </div>
</template>
