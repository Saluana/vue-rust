<script setup>
import Sib from './Sib.vue'
defineProps({ log: null })
</script>
<template>
  <div>
    <Sib name="a" :log="log" />
    <Sib name="b" :log="log" />
  </div>
</template>
