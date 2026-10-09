<script setup>
import { ref } from 'vue'
const s = ref('line1\nline2')
</script>
<template>
  <div>
    <textarea id="a" :value="s"></textarea>
    <textarea id="b">static text</textarea>
  </div>
</template>
