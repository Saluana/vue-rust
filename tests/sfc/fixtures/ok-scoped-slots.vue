<script setup>
const items = ['a', 'b']
</script>

<template>
  <ul>
    <li v-for="(item, i) in items" :key="item">
      <slot :item="item" :index="i">{{ item }}</slot>
    </li>
  </ul>
</template>
