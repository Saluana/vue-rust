<script setup>
const t = 1
</script>

<template>
  <div>
    <p :x.prop="t" :y.attr="t" data-allow-mismatch>z</p>
    <p v-html="t"></p>
  </div>
</template>
