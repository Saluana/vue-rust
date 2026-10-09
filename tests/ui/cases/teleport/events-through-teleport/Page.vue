<script setup>
defineEmits(['hit'])
</script>
<template>
  <div>
    <Teleport to="#overlay"><button id="x" @click="$emit('hit', 'teleported')">x</button></Teleport>
  </div>
</template>
