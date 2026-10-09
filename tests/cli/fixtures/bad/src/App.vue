<script setup lang="ts">
import { ref } from 'vue'
import lodash from 'lodash'

const n = ref(lodash.add(1, 2))
</script>

<template>
  <p>{{ n }}</p>
</template>
