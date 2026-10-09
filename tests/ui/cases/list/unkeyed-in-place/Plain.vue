<script setup>
defineProps({ n: Number })
</script>
<template>
  <div>
    <input v-for="i in n" :id="'f' + i" :data-i="i">
  </div>
</template>
