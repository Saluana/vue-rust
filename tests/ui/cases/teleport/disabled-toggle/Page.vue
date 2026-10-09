<script setup>
defineProps({ off: Boolean })
</script>
<template>
  <div>
    <Teleport to="#overlay" :disabled="off"><p id="t">moved</p></Teleport>
  </div>
</template>
