<script setup>
defineProps({
  count: { type: Number, default: 1 },
  flag: Boolean,
  list: { type: Array, default: () => ['d'] },
  cfg: { type: Object, default: () => ({ k: 'v' }) },
  label: { default: 'lbl' },
})
</script>
<template>
  <p id="p">{{ count }}|{{ flag }}|{{ list.join(',') }}|{{ cfg.k }}|{{ label }}</p>
</template>
