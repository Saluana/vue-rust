<script>
import { defineComponent, h } from 'vue'

export default defineComponent({
  name: 'Both',
  setup() {
    return () => h('p', { id: 'from-setup' }, 'from setup')
  },
})
</script>
<template>
  <p id="from-template">from template</p>
</template>
