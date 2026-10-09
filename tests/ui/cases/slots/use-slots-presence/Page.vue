<script setup>
import Pres from './Pres.vue'
</script>
<template>
  <div>
    <Pres />
    <Pres><template #footer>f</template></Pres>
  </div>
</template>
