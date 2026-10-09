// @tier 2
// @covers U09
// Radio buttons in one group bind a single string: checking the second deselects the first.
import Radio from './Radio.vue'
export async function run() {
  const m = testHost.mount(Radio)
  testHost.check(testHost.find(m, '#r2'), true)
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text(), testHost.find(m, '#r1').prop('checked'))
}
