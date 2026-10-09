<script setup>
defineProps({ log: null })
</script>
<template>
  <div>
    <Transition appear :css="false" @before-appear="log('before-appear')" @appear="(el, done) => { log('appear'); done() }" @after-appear="log('after-appear')">
      <p id="t">first</p>
    </Transition>
  </div>
</template>
