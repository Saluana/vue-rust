<script setup>
import { ref } from 'vue'
import Pair from './Pair.vue'
const a = ref('1')
const b = ref('2')
</script>
<template>
  <div>
    <Pair v-model:first="a" v-model:second="b" />
    <p id="out">{{ a }}|{{ b }}</p>
  </div>
</template>
