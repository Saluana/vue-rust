<script setup lang="ts">
import type { Missing } from './types-imported.ts'

// The type parameter Missing hides the import inside Box; the @vue-ignore member is not read (Vue's rule).
interface Box<Missing> {
  value: Missing
}

defineProps<Box<string> & { /* @vue-ignore */ flag: Missing }>()
</script>

<template><p>x</p></template>
