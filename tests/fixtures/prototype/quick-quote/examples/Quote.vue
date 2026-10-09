<script setup>
import { ref, computed } from 'vue'
const client = ref('Sample customer')
const job = ref('Hedge trim and garden cleanup')
const hours = ref(6)
const rate = ref(100)
const disposal = ref(150)
const materials = ref(0)
const labour = computed(() => hours.value * rate.value)
const total = computed(() => labour.value + disposal.value + materials.value)
const labourText = computed(() => labour.value.toFixed(2))
const totalText = computed(() => total.value.toFixed(2))
function hedgeRate() { rate.value = 100 }
function mowingRate() { rate.value = 80 }
function cleanupRate() { rate.value = 110 }
function clearExtras() { disposal.value = 0; materials.value = 0 }
</script>
<template>
  <main>
    <h1>Quick Quote</h1>
    <p>LAWN &amp; GARDEN / CAD ESTIMATE</p>
    <hr />
    <label>Customer</label>
    <input v-model="client" />
    <label>Job description</label>
    <input v-model="job" />
    <hr />
    <h2>Labour</h2>
    <label>Estimated hours</label>
    <input type="number" v-model.number="hours" min="0" max="1000" step="0.5" />
    <label>Hourly rate / CAD</label>
    <input type="number" v-model.number="rate" min="0" max="10000" step="5" />
    <button @click="hedgeRate">Hedging preset / $100 per hour</button>
    <button @click="mowingRate">Mowing preset / $80 per hour</button>
    <button @click="cleanupRate">Cleanup preset / $110 per hour</button>
    <hr />
    <h2>Extras</h2>
    <label>Green waste disposal / CAD</label>
    <input type="number" v-model.number="disposal" min="0" max="100000" step="25" />
    <label>Materials / CAD</label>
    <input type="number" v-model.number="materials" min="0" max="100000" step="10" />
    <button @click="clearExtras" :disabled="disposal === 0 &amp;&amp; materials === 0">Clear extras</button>
    <hr />
    <h2>Estimate / ${{ totalText }} CAD</h2>
    <p>Labour: ${{ labourText }} / {{ hours }} hours at ${{ rate }}</p>
    <p>Disposal: ${{ disposal }} / Materials: ${{ materials }}</p>
    <p v-if="hours === 0">Add hours to include labour in the estimate.</p>
    <p>Before tax. Editable sample rates. Not saved or sent.</p>
  </main>
</template>
