<script setup lang="ts">
interface Props {
  n?: number
  tag?: string
}

const props = withDefaults(defineProps<Props>(), { n: 3, tag: 'em' })
</script>

<template>
  <p>{{ props.tag }} {{ props.n }}</p>
</template>
