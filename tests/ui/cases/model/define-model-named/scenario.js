// @tier 2
// @covers U11,V26
// defineModel('title') binds the v-model:title argument, and the child's writes reach the parent.
import Parent from './Parent.vue'
export async function run() {
  const m = testHost.mount(Parent)
  testHost.input(testHost.find(m, '#n'), 'named')
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text())
}
