<script setup>
import Named from './Named.vue'
defineProps({ which: String })
</script>
<template>
  <Named :which="which">
    <template #[which]>from-parent</template>
  </Named>
</template>
