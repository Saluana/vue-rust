// @tier 2
// @covers U11,V26
// A built-in v-model modifier (.trim) reaches the child through defineModel's modifiers tuple, and the child's setter applies it.
import Parent from './Parent.vue'
export async function run() {
  const m = testHost.mount(Parent)
  testHost.input(testHost.find(m, '#m'), '  hi  ')
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text())
}
