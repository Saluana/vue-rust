<script setup>
import { ref, shallowRef, defineAsyncComponent } from 'vue'
import A from './A.vue'
import B from './B.vue'
import Lazy from './Lazy.vue'
const current = shallowRef(A)
const show = ref(false)
const shell = ref(true)
const phase = ref('idle')
const Async = defineAsyncComponent(() => Promise.resolve(Lazy))
</script>
<template>
  <main>
    <button id="swap-a" @click="current = A">A</button>
    <button id="swap-b" @click="current = B">B</button>
    <button id="toggle-motion" @click="show = !show">Motion</button>
    <button id="toggle-shell" @click="shell = !shell">Shell</button>
    <p id="phase">{{ phase }}</p>
    <div v-if="shell">
      <aside id="overlay"></aside>
      <Teleport defer to="#overlay"><button id="teleported" @click="phase = 'teleported'">Overlay</button></Teleport>
      <KeepAlive><component :is="current" /></KeepAlive>
      <Async />
      <Transition name="motion" @after-enter="phase = 'entered'"><p v-if="show" id="motion">Moving</p></Transition>
    </div>
  </main>
</template>
