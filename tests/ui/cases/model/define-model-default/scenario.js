// @tier 2
// @covers U11,V26
// defineModel() gives the child a writable ref bound to the parent's v-model.
import Parent from './Parent.vue'
export async function run() {
  const m = testHost.mount(Parent)
  testHost.input(testHost.find(m, '#d'), 'child-wrote')
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text())
}
