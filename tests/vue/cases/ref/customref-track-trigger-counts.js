// @tier 1
// @covers V11
// customRef's track and trigger are the only dependency hooks: a read outside an effect subscribes to nothing.
import {customRef, watchSyncEffect} from 'vue'
let gets = 0
let sets = 0
let stored = 0
let fire = () => {}
const c = customRef((track, trigger) => {
  fire = trigger
  return {
    get() {
      gets += 1
      track()
      return stored
    },
    set(v) {
      sets += 1
      stored = v
    },
  }
})
trace('read-outside', c.value, gets)
let runs = 0
watchSyncEffect(() => {
  c.value
  runs += 1
})
c.value = 4
trace('set-no-trigger', runs, sets, stored)
fire()
trace('after-trigger', runs, c.value, gets)
