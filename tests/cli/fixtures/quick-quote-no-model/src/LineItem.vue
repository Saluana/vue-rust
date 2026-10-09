<script setup lang="ts">
import FieldInput from './FieldInput.vue'
import { formatCents, lineTotal } from './money.ts'
import type { Line } from './money.ts'

const props = defineProps<{
  line: Line
  quantityError: string
  priceError: string
}>()

const emit = defineEmits<{
  edit: [id: number, field: string, text: string]
  remove: [id: number]
}>()

function onDescription(text: string) {
  emit('edit', props.line.id, 'description', text)
}

function onQuantity(text: string) {
  emit('edit', props.line.id, 'quantity', text)
}

function onPrice(text: string) {
  emit('edit', props.line.id, 'price', text)
}

function onRemove() {
  emit('remove', props.line.id)
}
</script>

<template>
  <li class="line" :id="'line-' + props.line.id">
    <FieldInput label="Description" :id="'description-' + props.line.id" :model-value="props.line.description" @update:model-value="onDescription" />
    <FieldInput label="Quantity" :id="'quantity-' + props.line.id" :model-value="String(props.line.quantity)" :error="props.quantityError" @update:model-value="onQuantity" />
    <FieldInput label="Unit price (USD)" :id="'price-' + props.line.id" :model-value="(props.line.unitCents / 100).toFixed(2)" :error="props.priceError" @update:model-value="onPrice" />
    <p class="subtotal">Subtotal: {{ formatCents(lineTotal(props.line)) }}</p>
    <button class="remove" :id="'remove-' + props.line.id" @click="onRemove">Remove</button>
  </li>
</template>
