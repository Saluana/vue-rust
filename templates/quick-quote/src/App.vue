<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import LineItem from './LineItem.vue'
import FieldInput from './FieldInput.vue'
import { formatCents, lineTotal, parseCents, parseQuantity } from './money.ts'
import type { Line } from './money.ts'

const customer = ref('')
const lines = ref<Line[]>([
  { id: 1, description: 'Widget', quantity: 2, unitCents: 1250 },
  { id: 2, description: 'Gadget', quantity: 1, unitCents: 499 },
])
const quantityErrors = ref<Record<number, string>>({})
const priceErrors = ref<Record<number, string>>({})
const nextId = ref(3)
const showForm = ref(true)
const status = ref('Quote ready')

const totalCents = computed(() => lines.value.reduce((sum, line) => sum + lineTotal(line), 0))
const total = computed(() => formatCents(totalCents.value))

watch(totalCents, (next, previous) => {
  status.value = `Total changed from ${formatCents(previous)} to ${formatCents(next)}`
})

function addLine() {
  lines.value = [...lines.value, { id: nextId.value, description: 'New item', quantity: 1, unitCents: 0 }]
  nextId.value++
}

function removeLine(id: number) {
  lines.value = lines.value.filter((line) => line.id !== id)
}

function editLine(id: number, field: string, text: string) {
  if (field === 'description') {
    lines.value = lines.value.map((line) => (line.id === id ? { ...line, description: text } : line))
  } else if (field === 'quantity') {
    const quantity = parseQuantity(text)
    quantityErrors.value = { ...quantityErrors.value, [id]: quantity === null ? 'Quantity must be a whole number of at least 1.' : '' }
    if (quantity !== null) {
      lines.value = lines.value.map((line) => (line.id === id ? { ...line, quantity } : line))
    }
  } else if (field === 'price') {
    const unitCents = parseCents(text)
    priceErrors.value = { ...priceErrors.value, [id]: unitCents === null ? 'Price must be zero or more dollars.' : '' }
    if (unitCents !== null) {
      lines.value = lines.value.map((line) => (line.id === id ? { ...line, unitCents } : line))
    }
  }
}
</script>

<template>
  <main class="quote">
    <h1>Quick quote</h1>
    <button id="toggle" @click="showForm = !showForm">{{ showForm ? 'Hide form' : 'Show form' }}</button>
    <section v-if="showForm" id="editor">
      <FieldInput label="Customer" id="customer" v-model="customer" />
      <ul id="lines">
        <LineItem
          v-for="line in lines"
          :key="line.id"
          :line="line"
          :quantity-error="quantityErrors[line.id] ?? ''"
          :price-error="priceErrors[line.id] ?? ''"
          @edit="editLine"
          @remove="removeLine"
        />
      </ul>
      <button id="add-line" @click="addLine">Add line</button>
    </section>
    <p id="total">Total: {{ total }}</p>
    <p id="status">{{ status }}</p>
  </main>
</template>

<style scoped>
.quote {
  font-family: sans-serif;
  max-width: 36rem;
  margin: 2rem auto;
}
.line {
  list-style: none;
  border: 1px solid #ccc;
  padding: 0.5rem;
  margin-bottom: 0.5rem;
}
.error {
  color: #b00020;
}
</style>
