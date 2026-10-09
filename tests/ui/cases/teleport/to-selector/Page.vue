<script setup>
import { ref } from 'vue'
const msg = ref('in overlay')
</script>
<template>
  <div>
    <Teleport to="#overlay"><p id="t">{{ msg }}</p></Teleport>
    <p id="local">here</p>
  </div>
</template>
