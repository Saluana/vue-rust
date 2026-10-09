<script setup>
defineProps({ map: Map })
</script>
<template>
  <div>
    <p v-for="[k, v] in map" :key="k">{{ k }}={{ v }}</p>
  </div>
</template>
