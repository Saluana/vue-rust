<script setup>
import { inject } from 'vue'
const v = inject('nothing')
</script>
<template>
  <p id="p">{{ v === undefined }}</p>
</template>
