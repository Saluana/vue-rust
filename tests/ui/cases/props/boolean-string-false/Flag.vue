<script setup>
defineProps({ flag: Boolean })
</script>
<template>
  <p id="p">{{ flag === true }}|{{ flag === false }}|{{ flag }}</p>
</template>
