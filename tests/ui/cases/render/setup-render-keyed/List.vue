<script>
import { defineComponent, h, ref } from 'vue'

export default defineComponent({
  name: 'Keyed',
  setup() {
    const items = ref(['a', 'b', 'c'])
    return () => h('div', null, [
      h('ul', { id: 'list' }, items.value.map((x) => h('li', { key: x, class: x }, x))),
      h('button', { id: 'rev', onClick: () => { items.value = [...items.value].reverse() } }, 'reverse'),
    ])
  },
})
</script>
