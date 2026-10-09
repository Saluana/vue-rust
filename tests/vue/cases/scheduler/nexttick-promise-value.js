// @tier 3
// @covers V17
// nextTick(fn) returns a promise that resolves with fn's result, after fn has run.
import {nextTick} from 'vue'
export async function run() {
  const v = await nextTick(() => 'value-from-fn')
  trace('resolved', v)
  const log = []
  const p = nextTick(() => {
    log.push('fn')
    return 7
  })
  log.push('sync')
  trace('ordered', await p, log)
}
