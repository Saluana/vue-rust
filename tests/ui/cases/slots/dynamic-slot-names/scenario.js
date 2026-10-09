// @tier 2
// @covers U12
// A dynamic slot name: the child asks for the slot named by its prop, and the parent supplies a slot with that name.
import Page from './Page.vue'
export async function run() {
  const m = testHost.mount(Page, { which: 'alpha' })
  trace('first', m.html())
  m.setProps({ which: 'beta' })
  await testHost.tick()
  trace('second', m.html())
}
