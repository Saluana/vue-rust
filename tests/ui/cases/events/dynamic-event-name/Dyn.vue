<script setup>
const props = defineProps({ evt: String })
const emit = defineEmits(['fired'])
function fire(e) {
  emit('fired', e.type)
}
</script>
<template>
  <input id="d" @[props.evt]="fire">
</template>
