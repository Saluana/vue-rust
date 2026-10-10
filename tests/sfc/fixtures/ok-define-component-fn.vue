<script>
import { defineComponent, h } from 'vue'

export default defineComponent((props, { attrs }) => () => h('i', { title: attrs.title }, 'fn'))
</script>
