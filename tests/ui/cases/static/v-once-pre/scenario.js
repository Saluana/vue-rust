// @tier 2
// @covers U27
// v-once keeps the first rendering; v-pre compiles nothing below it, so its moustaches stay as written.
import Once from './Once.vue'
export async function run() {
  const m = testHost.mount(Once, { msg: 'first' })
  m.setProps({ msg: 'second' })
  await testHost.tick()
  trace('html', m.html())
}
