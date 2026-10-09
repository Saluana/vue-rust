<script setup>
const props = defineProps({ show: Boolean, log: null, schedule: null })
function onEnter(el, done) {
  props.log('enter')
  props.schedule(30, done)
}
function onLeave(el, done) {
  props.log('leave')
  props.schedule(40, done)
}
</script>
<template>
  <div>
    <Transition :css="false" @before-enter="log('before-enter')" @enter="onEnter" @after-enter="log('after-enter')" @leave="onLeave" @after-leave="log('after-leave')">
      <p v-if="show" id="t">t</p>
    </Transition>
  </div>
</template>
