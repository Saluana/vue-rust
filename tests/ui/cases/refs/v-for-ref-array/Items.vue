<script setup>
import { ref, onMounted } from 'vue'
const props = defineProps({ log: null })
const items = ref([])
onMounted(() => props.log('count ' + items.value.length + ' ' + items.value.map((el) => el.id).join(',')))
</script>
<template>
  <ul>
    <li v-for="i in 3" :key="i" :id="'i' + i" ref="items">{{ i }}</li>
  </ul>
</template>
