<script setup>
defineProps({ show: Boolean })
</script>
<template>
  <div>
    <template v-if="show">
      <p>one</p>
      <p>two</p>
    </template>
    <span>end</span>
  </div>
</template>
