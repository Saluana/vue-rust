<script setup>
const emit = defineEmits(['pick'])
function report(v, e) {
  emit('pick', v, e.type, e.button)
}
</script>
<template>
  <div>
    <button id="a" @click="$emit('pick', 'a', $event.type)">a</button>
    <button id="b" @click="report('b', $event)">b</button>
  </div>
</template>
