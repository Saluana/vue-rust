<script setup lang="ts">
import type { ImportedProps } from './types-imported.ts'

const props = defineProps<ImportedProps>()
</script>

<template>
  <p>{{ props.label }} {{ props.count }} {{ props.enabled ? 'on' : 'off' }}</p>
</template>
