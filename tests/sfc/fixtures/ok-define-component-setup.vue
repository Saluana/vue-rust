<script>
import { defineComponent, h } from 'vue'

export default defineComponent({
  name: 'Greeting',
  inheritAttrs: false,
  setup() {
    return () => h('p', { class: 'hi' }, 'hi')
  },
})
</script>
