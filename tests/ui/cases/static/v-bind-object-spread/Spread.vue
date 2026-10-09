<script setup>
defineProps({ opts: Object })
</script>
<template>
  <a class="base" v-bind="opts" :class="{ on: opts.on }">x</a>
</template>
