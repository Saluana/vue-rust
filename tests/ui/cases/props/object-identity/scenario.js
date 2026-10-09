// @tier 1
// @covers U11
// Props hold the caller's object itself, not a copy: the child sees the identical object.
import Id from './Id.vue'
export function run() {
  const obj = { k: 'v' }
  const m = testHost.mount(Id, { obj, expect: obj, log: (...v) => trace('log', ...v) })
  trace('text', testHost.find(m, '#p').text())
}
