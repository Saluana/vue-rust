// @tier 1
// @covers U19
// A dynamic <component> with a native tag name renders that element.
import Native from './Native.vue'
export async function run() {
  const m = testHost.mount(Native, { tag: 'span' })
  trace('span', m.html())
  m.setProps({ tag: 'em' })
  await testHost.tick()
  trace('em', m.html())
}
