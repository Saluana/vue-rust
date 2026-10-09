// @tier 3
// @covers V11
// customRef with explicit track and trigger: a write lands after a 10 ms delay on the fake clock, and only then notifies.
import {customRef, watchSyncEffect} from 'vue'
function debounced(initial, delay) {
  let value = initial
  let timer = 0
  return customRef((track, trigger) => ({
    get() {
      track()
      return value
    },
    set(next) {
      testHost.timers.clearTimeout(timer)
      timer = testHost.timers.setTimeout(() => {
        value = next
        trigger()
      }, delay)
    },
  }))
}
const text = debounced('a', 10)
const seen = []
watchSyncEffect(() => seen.push(text.value))
export async function run() {
  text.value = 'b'
  text.value = 'c'
  await testHost.timers.advance(5)
  trace('before-delay', seen, text.value)
  await testHost.timers.advance(10)
  trace('after-delay', seen, text.value, testHost.timers.now())
}
