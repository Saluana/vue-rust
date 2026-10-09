// @tier 1
// @covers V05
// shallowReactive tracks only its own properties. A nested object stays raw, so its own mutations are not tracked.
import {isReactive, shallowReactive, watchSyncEffect} from 'vue'
const inner = {n: 1}
const s = shallowReactive({inner, count: 0})
let runs = 0
watchSyncEffect(() => {
  s.inner.n
  runs += 1
})
s.inner.n = 2
trace('nested', runs, isReactive(s.inner), s.inner === inner)
s.inner = {n: 9}
trace('root-replace', runs, s.inner.n)
