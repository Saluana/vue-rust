<script setup>
import Counter from './Counter.vue'
defineProps({ show: Boolean })
</script>
<template>
  <div><Counter v-if="show" /></div>
</template>
