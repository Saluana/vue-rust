// @tier 2
// @covers U09
// v-model on a single select: the change event stores the value of the chosen option.
import Single from './Single.vue'
export async function run() {
  const m = testHost.mount(Single)
  testHost.select(testHost.find(m, '#s'), 'b')
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text())
}
