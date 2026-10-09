// @tier 2
// @covers U18
// An element ref keeps pointing at the same element across updates of its text.
import { ref } from 'vue'
import Keep from './Keep.vue'
export async function run() {
  const reg = {}
  const m = testHost.mount(Keep, { n: 1, log: () => {} })
  const before = testHost.find(m, 'p')
  m.setProps({ n: 2, log: () => {} })
  await testHost.tick()
  const after = testHost.find(m, 'p')
  trace('text', after.text(), after.exists(), before.exists())
}
