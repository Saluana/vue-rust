<script setup>
const props = defineProps({ which: String, v: String, log: null })
const vDemo = {
  updated(el, b) {
    props.log(b.arg + '=' + b.value)
  },
}
</script>
<template>
  <p v-demo:[which]="v">x</p>
</template>
