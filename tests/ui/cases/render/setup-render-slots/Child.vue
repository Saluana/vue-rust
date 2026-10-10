<script setup>
defineProps({ msg: String })
</script>
<template>
  <p class="msg">{{ msg }} <slot /></p>
  <i class="footer"><slot name="footer" /></i>
</template>
