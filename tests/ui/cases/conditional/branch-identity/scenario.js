// @tier 2
// @covers U14
// Switching v-if branches creates a new element: the old one is released (its ref becomes null) and the new one is a different node.
import Branch from './Branch.vue'
export async function run() {
  const reg = {}
  const m = testHost.mount(Branch, { on: true, reg })
  const first = reg.a
  m.setProps({ on: false, reg })
  await testHost.tick()
  trace('html', m.html())
  trace('released', reg.a === null, reg.b !== first, reg.b !== undefined)
}
