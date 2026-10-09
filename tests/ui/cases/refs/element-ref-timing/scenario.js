// @tier 1
// @covers U18,V27
// A template ref is null during setup and holds the element from mounted onwards.
import Box from './Box.vue'
export function run() {
  testHost.mount(Box, { log: (v) => trace('log', v) })
}
