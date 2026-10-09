<script setup>
import { ref } from 'vue'
const items = ref(['x', 1])
const obj = { k: 'v' }
const s = '<i>not markup</i>'
</script>
<template>
  <div>
    <p id="a">{{ items }}</p>
    <p id="b">{{ obj }}</p>
    <p id="c">{{ null }}|{{ 0 }}|{{ false }}</p>
    <p id="d" v-text="s"></p>
    <p id="e">{{ s }}</p>
  </div>
</template>
