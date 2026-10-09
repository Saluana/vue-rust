<script setup>
defineProps(['v'])
</script>
<template>
  <div>
    <p v-if="v">truthy</p>
    <p v-else>falsy</p>
  </div>
</template>
