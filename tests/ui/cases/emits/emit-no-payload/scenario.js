// @tier 1
// @covers U11
// An emit without a payload calls the listener with no arguments.
import Np from './Np.vue'
export function run() {
  const m = testHost.mount(Np, { onSave: (...v) => trace('save', v.length) })
  testHost.click(testHost.find(m, '#n'))
}
