// @tier 1
// @covers V01
// ref() deep-wraps a plain object: .value is a reactive proxy, and a nested write reruns effects that read it.
import {isReactive, ref, toRaw, watchSyncEffect} from 'vue'
const raw = {n: 1, inner: {m: 1}}
const r = ref(raw)
let runs = 0
watchSyncEffect(() => {
  r.value.inner.m
  runs += 1
})
r.value.inner.m = 2
trace('deep', isReactive(r.value), isReactive(r.value.inner), toRaw(r.value) === raw, runs, raw.inner.m)
