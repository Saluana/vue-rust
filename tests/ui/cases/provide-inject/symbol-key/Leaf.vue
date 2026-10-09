<script setup>
import { inject } from 'vue'
const props = defineProps({ k: null })
const v = inject(props.k)
</script>
<template>
  <p id="p">{{ v }}</p>
</template>
