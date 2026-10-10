import { defineComponent, h, ref } from 'vue'

// A .js module: the same defineComponent setup, returning a render function that reads its own state.
export default defineComponent({
  name: 'FromJs',
  setup() {
    const count = ref(0)
    return () => h('button', { type: 'button', id: 'js', onClick: () => { count.value += 1 } }, `js ${count.value}`)
  },
})
