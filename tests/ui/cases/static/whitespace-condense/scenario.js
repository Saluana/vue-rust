// @tier 1
// @covers U30
// Compiler whitespace policy "condense": whitespace between elements that contains a line break is removed, other runs collapse to one space.
import Ws from './Ws.vue'
export function run() {
  trace('html', testHost.mount(Ws).html())
}
