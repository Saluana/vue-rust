<script setup>
import { ref } from 'vue'
import Mod from './Mod.vue'
const t = ref('')
</script>
<template>
  <div>
    <Mod v-model.trim="t" />
    <p id="out">{{ t }}</p>
  </div>
</template>
