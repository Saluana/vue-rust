<script setup>
defineProps({ handlers: Object })
</script>
<template>
  <button id="o" v-on="handlers">o</button>
</template>
