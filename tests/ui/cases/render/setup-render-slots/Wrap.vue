<script>
import { defineComponent, h } from 'vue'
import Child from './Child.vue'

export default defineComponent({
  name: 'Wrap',
  setup() {
    return () => h('div', { id: 'wrap' }, [
      h(Child, { msg: 'hello' }, { default: () => h('span', 'slot'), footer: () => h('b', 'foot') }),
      h(Child, { msg: 'bare' }),
    ])
  },
})
</script>
