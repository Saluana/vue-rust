<script setup lang="ts">
import type { AliasProps } from './types-imported.ts'

const props = withDefaults(defineProps<AliasProps>(), { title: 'Untitled', size: 2 })
</script>

<template>
  <h2>{{ props.title }} {{ props.size }} {{ props.open ? 'open' : 'closed' }}</h2>
</template>
