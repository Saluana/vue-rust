<script setup>
defineProps({ which: String })
</script>
<template>
  <div>
    <slot :name="which">fb</slot>
  </div>
</template>
