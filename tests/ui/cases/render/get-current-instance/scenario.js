// @tier 1
// @covers A18
// getCurrentInstance returns the same handle on every call within one component's setup, a different handle for its child, and null outside setup.
import {getCurrentInstance} from 'vue'
import Parent from './Parent.vue'
export function run() {
  trace('outside', getCurrentInstance() === null)
  const log = []
  testHost.mount(Parent, {log})
  trace('log', log)
}
