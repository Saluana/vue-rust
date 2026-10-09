<script setup>
import Kebab from './Kebab.vue'
defineProps({ log: null })
</script>
<template>
  <div>
    <Kebab @some-event="(v) => log('kebab', v)" />
  </div>
</template>
