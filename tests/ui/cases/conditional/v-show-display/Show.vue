<script setup>
defineProps({ show: Boolean })
</script>
<template>
  <div>
    <p id="p" v-show="show">visible</p>
    <input id="i" v-show="show" type="text">
  </div>
</template>
