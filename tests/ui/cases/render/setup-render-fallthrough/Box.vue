<script>
import { defineComponent, h } from 'vue'

export default defineComponent({
  name: 'Box',
  setup(props, { attrs }) {
    return () => h('div', { class: 'box', 'data-kind': attrs.kind }, `kind ${attrs.kind}`)
  },
})
</script>
