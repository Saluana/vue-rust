<script setup>
defineProps({ on: Boolean, reg: Object })
</script>
<template>
  <div>
    <input v-if="on" :ref="(el) => { reg.a = el }" id="x" value="a">
    <input v-else :ref="(el) => { reg.b = el }" id="x" value="b">
  </div>
</template>
