<script setup>
defineProps({ current: null, log: null })
</script>
<template>
  <div>
    <KeepAlive include="A">
      <component :is="current" :log="log" />
    </KeepAlive>
  </div>
</template>
