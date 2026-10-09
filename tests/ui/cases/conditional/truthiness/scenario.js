// @tier 2
// @covers U14
// v-if uses JavaScript truthiness: '', 0, NaN and null are false; an empty array and the string '0' are true.
import Truth from './Truth.vue'
export async function run() {
  const m = testHost.mount(Truth, { v: '' })
  const seen = [m.html().includes('falsy')]
  for (const v of [0, NaN, null, [], '0', 1]) {
    m.setProps({ v })
    await testHost.tick()
    seen.push(m.html().includes('truthy'))
  }
  trace('truthy-flags', seen)
}
