// @tier 1
// @covers V01
// ref(reactiveObject) keeps that proxy as .value; ref(existingRef) returns the same ref; writing the same proxy is no change.
import {isRef, reactive, ref, watchSyncEffect} from 'vue'
const o = reactive({n: 1})
const r = ref(o)
trace('proxy-kept', r.value === o)
const again = ref(r)
trace('ref-of-ref', again === r, isRef(again))
let runs = 0
watchSyncEffect(() => {
  r.value
  runs += 1
})
r.value = o
trace('same-proxy-write', runs)
