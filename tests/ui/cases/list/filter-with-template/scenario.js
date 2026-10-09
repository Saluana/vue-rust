// @tier 2
// @covers U16,U14
// A keyed <template v-for> around a v-if: items that fail the condition leave a comment placeholder in their slot.
import Filter from './Filter.vue'
export async function run() {
  const m = testHost.mount(Filter, { items: [{ id: 1, on: true }, { id: 2, on: false }, { id: 3, on: true }] })
  trace('html', m.snapshot())
  m.setProps({ items: [{ id: 1, on: false }, { id: 2, on: true }, { id: 3, on: true }] })
  await testHost.tick()
  trace('html2', m.html())
}
