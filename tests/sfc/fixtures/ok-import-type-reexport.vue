<script setup lang="ts">
import type { Props, AliasProps } from './types-reexport.ts'

const props = defineProps<Props & AliasProps>()
</script>

<template>
  <p>{{ props.label }} {{ props.title }}</p>
</template>
