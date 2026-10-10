<script>
import { defineComponent, h } from 'vue'

export default defineComponent({
  name: 'FromVue',
  setup() {
    return () => h('p', { id: 'vue' }, 'from vue')
  },
})
</script>
