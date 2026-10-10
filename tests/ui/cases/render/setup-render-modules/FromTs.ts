import { defineComponent, h, ref } from 'vue'

// A .ts module with no type syntax, so Chromium runs the file as written; typed .ts forms are not exercised by these tests.
export default defineComponent({
  name: 'FromTs',
  setup() {
    const label = 'ts'
    const count = ref(0)
    return () => h('button', { type: 'button', id: 'ts', onClick: () => { count.value += 1 } }, `${label} ${count.value}`)
  },
})
