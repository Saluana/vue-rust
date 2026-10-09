<script setup>
import { useSlots } from 'vue'
const slots = useSlots()
</script>
<template>
  <p id="p">{{ slots.footer ? 'footer' : 'none' }}|{{ slots.default ? 'body' : 'empty' }}</p>
</template>
