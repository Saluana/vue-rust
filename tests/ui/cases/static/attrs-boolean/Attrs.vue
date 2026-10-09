<script setup>
defineProps({ off: Boolean, title: String, hidden: Boolean })
</script>
<template>
  <div>
    <input id="i" :disabled="off" required>
    <button id="b" disabled>x</button>
    <p id="p" :title="title" data-x="1" :hidden="hidden" :aria-busy="off">t</p>
  </div>
</template>
