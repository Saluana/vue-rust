<script setup>
import { defineProps } from 'vue'
const props = defineProps({ a: String })
</script>
<template><p>{{ props.a }}</p></template>
