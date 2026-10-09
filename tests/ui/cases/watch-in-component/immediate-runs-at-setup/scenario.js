// @tier 1
// @covers V14
// An immediate watcher calls its callback during setup, before the first render is complete.
import Imm from './Imm.vue'
export function run() {
  trace('before')
  testHost.mount(Imm, { n: 4, log: (v) => trace('log', v) })
  trace('after')
}
