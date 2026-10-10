<script>
import { defineComponent, h, ref } from 'vue'

export default defineComponent({
  name: 'Counter',
  setup() {
    const count = ref(0)
    return () => h('button', { type: 'button', id: 'inc', onClick: () => { count.value += 1 } }, `clicks ${count.value}`)
  },
})
</script>
