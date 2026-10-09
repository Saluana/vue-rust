<script setup>
import Fb from './Fb.vue'
</script>
<template>
  <div>
    <Fb><template #default="{ x }">got {{ x }}</template></Fb>
    <Fb />
  </div>
</template>
