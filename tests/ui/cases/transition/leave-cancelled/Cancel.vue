<script setup>
const props = defineProps({ show: Boolean, log: null, schedule: null })
function onLeave(el, done) {
  props.log('leave')
  props.schedule(50, done)
}
</script>
<template>
  <div>
    <Transition :css="false" @leave="onLeave" @leave-cancelled="log('leave-cancelled')" @enter="(el, done) => { log('enter'); done() }">
      <p v-if="show" id="t">t</p>
    </Transition>
  </div>
</template>
