<script setup>
import { defineComponent, h, ref } from 'vue'
import Fn from './Fn.vue'

const Greet = defineComponent(function Greet() {
  return () => h('em', 'greet')
})
const Arrow = defineComponent(() => () => h('b', 'arrow'))
// The extra options argument is merged into the component: inheritAttrs false keeps class off the root.
const Quiet = defineComponent((props, { attrs }) => () => h('u', `quiet ${attrs.class}`), { inheritAttrs: false })
const show = ref(true)
</script>
<template>
  <div>
    <Greet />
    <Arrow v-if="show" />
    <Fn title="T" />
    <Quiet class="outer" />
    <button id="toggle" type="button" @click="show = !show">toggle</button>
  </div>
</template>
