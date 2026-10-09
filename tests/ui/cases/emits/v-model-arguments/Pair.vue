<script setup>
const first = defineModel('first')
const second = defineModel('second')
</script>
<template>
  <div>
    <input id="x" v-model="first">
    <input id="y" v-model="second">
  </div>
</template>
