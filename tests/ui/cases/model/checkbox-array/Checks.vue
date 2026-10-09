<script setup>
import { ref } from 'vue'
const picked = ref(['b'])
const flag = ref(false)
const tv = ref('no')
</script>
<template>
  <div>
    <input id="a" type="checkbox" value="a" v-model="picked">
    <input id="b" type="checkbox" value="b" v-model="picked">
    <input id="c" type="checkbox" value="c" v-model="picked">
    <input id="f" type="checkbox" v-model="flag">
    <input id="tv" type="checkbox" v-model="tv" true-value="yes" false-value="no">
    <p id="out">{{ picked.join(',') }}|{{ flag }}|{{ tv }}</p>
  </div>
</template>
