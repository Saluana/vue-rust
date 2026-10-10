<script setup lang="ts">
import type { Base } from './types-imported.ts'

interface Local extends Base {
  own: string
}
defineProps<Local>()
</script>

<template><p>x</p></template>
