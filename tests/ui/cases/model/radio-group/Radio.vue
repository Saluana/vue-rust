<script setup>
import { ref } from 'vue'
const picked = ref('one')
</script>
<template>
  <div>
    <input id="r1" type="radio" name="g" value="one" v-model="picked">
    <input id="r2" type="radio" name="g" value="two" v-model="picked">
    <p id="out">{{ picked }}</p>
  </div>
</template>
