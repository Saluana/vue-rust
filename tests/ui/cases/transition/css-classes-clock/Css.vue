<script setup>
defineProps({ show: Boolean })
</script>
<template>
  <div>
    <Transition>
      <p v-if="show" id="t">t</p>
    </Transition>
  </div>
</template>
