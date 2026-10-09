<script setup>
import { ref } from 'vue'
const msg = ref('')
const padded = ref('')
const lazy = ref('')
const num = ref(0)
</script>
<template>
  <div>
    <input id="t" v-model="msg">
    <input id="tr" v-model.trim="padded">
    <input id="lz" v-model.lazy="lazy">
    <input id="nm" v-model.number="num">
    <p id="out">{{ msg }}|{{ padded }}|{{ lazy }}|{{ num }}</p>
  </div>
</template>
