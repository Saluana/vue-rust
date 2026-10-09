// @tier 3
// @covers V17
// A nextTick callback that throws rejects the promise nextTick returned. The caller can catch that rejection.
import {nextTick} from 'vue'
export async function run() {
  let boom = null
  try {
    await nextTick(() => {
      boom = new Error('in tick')
      throw boom
    })
    trace('not-reached', true)
  } catch (e) {
    trace('caught', e === boom)
  }
}
