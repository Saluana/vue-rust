<script setup>
defineProps({ on: Boolean })
</script>
<template>
  <div>
    <input id="c" type="checkbox" :checked="on">
  </div>
</template>
