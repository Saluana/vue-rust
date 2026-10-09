// @tier 3
// @covers V17
// With nothing pending, nextTick() returns the same resolved promise each time. It resolves after the promise jobs already queued.
import {nextTick} from 'vue'
const p1 = nextTick()
const p2 = nextTick()
trace('same-promise', p1 === p2)
const log = []
new Promise((resolve) => resolve()).then(() => log.push('queued-first'))
export async function run() {
  await nextTick()
  log.push('after')
  trace('order', log)
}
