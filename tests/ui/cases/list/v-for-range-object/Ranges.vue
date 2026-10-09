<script setup>
defineProps({ obj: Object, n: Number, text: String })
</script>
<template>
  <div>
    <span v-for="i in n" :key="i">{{ i }}</span>
    <em v-for="(v, k, idx) in obj" :key="k">{{ k }}={{ v }}@{{ idx }}</em>
    <b v-for="ch in text">{{ ch }}</b>
  </div>
</template>
