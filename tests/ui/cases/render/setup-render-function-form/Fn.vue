<script>
import { defineComponent, h } from 'vue'

// The function form in a default export: the function is the setup, and its result is the render.
export default defineComponent((props, { attrs }) => () => h('i', { title: attrs.title }, 'fn'))
</script>
