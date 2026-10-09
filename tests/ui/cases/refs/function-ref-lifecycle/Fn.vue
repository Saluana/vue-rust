<script setup>
const props = defineProps({ log: null, show: Boolean })
function track(el) {
  props.log(el === null ? 'null' : 'set ' + el.tagName)
}
</script>
<template>
  <div>
    <p v-if="show" :ref="track">p</p>
  </div>
</template>
