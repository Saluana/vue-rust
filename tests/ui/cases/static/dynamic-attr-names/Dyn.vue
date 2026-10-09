<script setup>
import { ref } from 'vue'
defineProps({ name: String, val: String })
const id = ref('same')
</script>
<template>
  <div :[name]="val" :id>{{ id }}</div>
</template>
