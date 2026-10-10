<script setup lang="ts">
import type { Dayjs } from 'dayjs'
import type * as Types from './types-imported.ts'

const emit = defineEmits<{ change: [when: Dayjs]; load: [props: Types.ImportedProps] }>()
</script>

<template><p>x</p></template>
