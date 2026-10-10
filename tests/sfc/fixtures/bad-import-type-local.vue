<script setup lang="ts">
import type { Missing } from './types-imported.ts'

type Local = { flag: Missing }
defineProps<Local>()
</script>

<template><p>x</p></template>
