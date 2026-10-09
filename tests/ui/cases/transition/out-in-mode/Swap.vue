<script setup>
const props = defineProps({ name: String, log: null, schedule: null })
function onLeave(el, done) {
  props.log('leave ' + el.textContent)
  props.schedule(20, done)
}
function onEnter(el, done) {
  props.log('enter ' + el.textContent)
  props.schedule(20, done)
}
</script>
<template>
  <div>
    <Transition mode="out-in" :css="false" @leave="onLeave" @enter="onEnter">
      <p :key="name">{{ name }}</p>
    </Transition>
  </div>
</template>
