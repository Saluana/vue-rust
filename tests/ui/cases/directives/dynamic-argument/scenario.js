// @tier 2
// @covers U26
// A dynamic directive argument is read on each update, so the same directive reports the argument it is given now.
import Dyn from './Dyn.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Dyn, { which: 'alpha', v: '1', log })
  m.setProps({ which: 'beta', v: '2', log })
  await testHost.tick()
  trace('html', m.html())
}
