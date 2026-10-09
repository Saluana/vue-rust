<script setup>
defineProps({ rows: Array })
</script>
<template>
  <section>
    <div v-for="row in rows" :key="row.id" class="row">
      <span v-for="cell in row.cells" :key="cell">{{ row.id }}.{{ cell }}</span>
    </div>
  </section>
</template>
