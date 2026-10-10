<script setup lang="ts">
import { ref } from 'vue'
import Child from './Child.vue'

const count = ref(0)
count.value = 'three'
</script>

<template>
  <p>{{ count }}</p>
  <Child :label="3" />
</template>
