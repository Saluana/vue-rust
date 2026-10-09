// @tier 1
// @covers V18,U18
// The template ref is set before mounted runs, so the hook can read the element.
import Box from './Box.vue'
export function run() {
  testHost.mount(Box, { log: (v) => trace('log', v) })
}
