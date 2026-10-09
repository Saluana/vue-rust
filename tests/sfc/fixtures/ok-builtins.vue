<script setup>
import { ref } from 'vue'

const on = ref(true)
</script>

<template>
  <div>
    <Transition><p v-if="on">x</p></Transition>
    <KeepAlive><span>k</span></KeepAlive>
    <Teleport to="body"><i>t</i></Teleport>
    <transition-group tag="ul"><li v-for="n in 2" :key="n">{{ n }}</li></transition-group>
  </div>
</template>
