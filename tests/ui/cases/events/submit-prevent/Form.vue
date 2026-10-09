<script setup>
defineEmits(['ev'])
</script>
<template>
  <form id="f" @submit.prevent="$emit('ev', 'submit')">
    <button>go</button>
  </form>
</template>
