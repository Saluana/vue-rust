<script setup>
defineProps({ current: null, log: null })
</script>
<template>
  <div>
    <KeepAlive :max="1">
      <component :is="current" :log="log" />
    </KeepAlive>
  </div>
</template>
