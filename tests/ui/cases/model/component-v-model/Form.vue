<script setup>
import { ref } from 'vue'
import Field from './Field.vue'
const text = ref('')
</script>
<template>
  <div>
    <Field v-model="text" />
    <p id="out">{{ text }}</p>
  </div>
</template>
