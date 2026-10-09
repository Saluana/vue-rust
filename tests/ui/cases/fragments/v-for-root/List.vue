<script setup>
defineProps({ n: Number })
</script>
<template>
  <li v-for="i in n" :key="i">{{ i }}</li>
</template>
