<script>
import { defineComponent, h } from 'vue'

export default defineComponent({
  name: 'Quiet',
  inheritAttrs: false,
  setup(props, { attrs }) {
    return () => h('p', { class: 'quiet' }, `title ${attrs.title}`)
  },
})
</script>
