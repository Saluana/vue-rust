<script setup>
defineEmits(['hit'])
</script>
<template>
  <div id="outer" @click="$emit('hit', 'outer')">
    <button id="inner" @click.stop="$emit('hit', 'inner')">x</button>
    <span id="self" @click.self="$emit('hit', 'self')">y</span>
    <a id="link" href="#" @click.prevent="$emit('hit', 'link')">l</a>
    <i id="once" @click.once="$emit('hit', 'once')">o</i>
  </div>
</template>
