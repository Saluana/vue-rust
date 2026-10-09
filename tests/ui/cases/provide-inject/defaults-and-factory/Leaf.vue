<script setup>
import { inject } from 'vue'
const dflt = inject('missing', 'dflt')
const fac = inject('fac', () => ({ v: 1 }), true)
const plain = inject('plain', 42)
</script>
<template>
  <p id="p">{{ dflt }}|{{ fac.v }}|{{ plain }}</p>
</template>
