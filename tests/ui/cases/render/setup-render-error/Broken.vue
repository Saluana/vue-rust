<script>
import { defineComponent } from 'vue'

export default defineComponent({
  name: 'Broken',
  setup() {
    return () => {
      throw new Error('render failed')
    }
  },
})
</script>
