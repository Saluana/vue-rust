<script setup>
import Scope from './Scope.vue'
</script>
<template>
  <Scope>
    <template #default="{ item = 'none' }">{{ item }}</template>
  </Scope>
</template>
