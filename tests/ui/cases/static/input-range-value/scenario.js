// @tier 2
// @covers U10
// A range input sanitizes its value to [min, max]: the DOM value is 10 while the model keeps 20, and the snapshot shows the dirty value prop.
// A range is set with input(), the event v-model on a range listens to (change does not update it).
import Range from './Range.vue'
export async function run() {
  const m = testHost.mount(Range)
  trace('snapshot', m.snapshot())
  testHost.input(testHost.find(m, '#s'), '7')
  await testHost.tick()
  trace('text', testHost.find(m, '#out').text())
}
