// @tier 1
// @covers U01
// Interpolation stringifies values like JSON for objects and arrays, prints null as empty, and never turns text into markup.
import Values from './Values.vue'
export function run() {
  const m = testHost.mount(Values)
  trace('html', m.html())
  trace('texts', testHost.find(m, '#d').text(), testHost.find(m, '#e').text())
}
