<script>
import { defineComponent, h } from 'vue'

export default defineComponent({
  name: 'Tree',
  setup() {
    return () => h('section', { id: 'top', class: ['card', { wide: true, tall: false }], style: { color: 'red', margin: '2px' } }, [
      h('h2', { title: 'heading' }, 'Title'),
      h('p', null, ['first ', 'second']),
      h('ul', null, [h('li', 'a'), h('li', 'b')]),
      'trailing text',
    ])
  },
})
</script>
