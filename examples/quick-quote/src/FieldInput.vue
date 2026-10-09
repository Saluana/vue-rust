<script setup lang="ts">
// A labelled text input. The value is a two-way model (defineModel); the error line is shown only when set.
const props = defineProps<{
  label: string
  id: string
  error?: string
}>()

const value = defineModel<string>({ default: '' })
</script>

<template>
  <label class="field">
    <span>{{ props.label }}</span>
    <input :id="props.id" v-model="value" />
  </label>
  <p v-if="props.error" class="error">{{ props.error }}</p>
</template>
