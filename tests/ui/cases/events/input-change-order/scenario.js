// @tier 2
// @covers U05
// A typed value fires input first; change follows when the field is committed.
import Field from './Field.vue'
export function run() {
  const m = testHost.mount(Field, { onEv: (v) => trace('ev', v) })
  testHost.input(testHost.find(m, '#f'), 'x')
  testHost.change(testHost.find(m, '#f'), 'x')
}
