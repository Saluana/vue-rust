<script setup lang="ts">
// Test fixture: examples/quick-quote with FieldInput written with defineProps and defineEmits instead of defineModel,
// because the runtime does not export useModel yet. Every other file is identical to the example.
const props = defineProps<{
  label: string
  id: string
  modelValue: string
  error?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

function onInput(event: Event) {
  emit('update:modelValue', (event.target as HTMLInputElement).value)
}
</script>

<template>
  <label class="field">
    <span>{{ props.label }}</span>
    <input :id="props.id" :value="props.modelValue" @input="onInput" />
  </label>
  <p v-if="props.error" class="error">{{ props.error }}</p>
</template>
