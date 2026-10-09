<script setup>
import { ref } from 'vue'

const props = defineProps({ label: { type: String, default: 'none' }, size: Number })
const emit = defineEmits(['ping'])
const pings = ref(0)

function ping() {
  pings.value++
  emit('ping', pings.value)
}
</script>

<template>
  <div>
    <button class="ping" @click="ping">{{ props.label }} {{ pings }}</button>
  </div>
</template>
