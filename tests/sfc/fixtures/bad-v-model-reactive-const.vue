<script setup>
import { reactive } from 'vue'

const form = reactive({ name: '' })
</script>

<template>
  <input type="text" v-model="form">
</template>
