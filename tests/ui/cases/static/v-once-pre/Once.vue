<script setup>
defineProps({ msg: String })
</script>
<template>
  <div>
    <p id="once" v-once>{{ msg }}</p>
    <p id="pre" v-pre>{{ msg }} raw</p>
    <p id="live">{{ msg }}</p>
  </div>
</template>
