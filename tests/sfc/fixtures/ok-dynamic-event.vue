<script setup>
import { ref } from 'vue'

const evt = 'click'
const attr = 'title'
const hits = ref(0)
</script>

<template>
  <button @[evt]="hits++" :[attr]="hits">{{ hits }}</button>
</template>
