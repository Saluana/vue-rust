<script setup>
const props = defineProps({ obj: Object, expect: Object, log: null })
props.log('same-object', props.obj === props.expect)
</script>
<template>
  <p id="p">{{ obj.k }}</p>
</template>
