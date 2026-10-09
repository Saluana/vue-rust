<script setup>
import { ref } from 'vue'
const p = ref(null)
defineProps({ show: Boolean, log: null })
</script>
<template>
  <div>
    <p v-if="show" ref="p" id="p">p</p>
  </div>
</template>
