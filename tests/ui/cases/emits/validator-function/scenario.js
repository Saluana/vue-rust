// @tier 1
// @covers U11
// An emit validator that returns false warns only in development: the production build still calls the listener.
import Val from './Val.vue'
export function run() {
  const m = testHost.mount(Val, { onSave: (p) => trace('save', p) })
  testHost.click(testHost.find(m, '#b'))
}
