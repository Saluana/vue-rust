<script setup>
import { defineComponent, ref } from 'vue'

const Card = defineComponent({ render() { return 'card' } })
const name = ref('header')
</script>

<template>
  <div>
    <Card>
      <template #header="{ title }"><b>{{ title }}</b></template>
      <template #[name]><i>dynamic</i></template>
    </Card>
  </div>
</template>
