// @tier 1
// @covers V27,U18
// defineExpose limits the parent's view of a child: only the exposed names are reachable through the template ref.
import Parent from './Parent.vue'
export function run() {
  testHost.mount(Parent, { log: (v) => trace('log', v) })
}
