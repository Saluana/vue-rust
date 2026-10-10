// @tier 1
// @covers V02,V04
// triggerRef on a readonly shallowRef reruns the subscribers of the ref, as triggerRef on the ref itself does. readonly() does
// not wrap the ref's dependency, so the trigger reaches the effect and the computed that read the ref through the proxy.
import {computed, effect, readonly, shallowRef, triggerRef} from 'vue'
const r = shallowRef({a: 1})
const ror = readonly(r)
let dummy = 0
let effectRuns = 0
effect(() => {
  effectRuns += 1
  dummy = ror.value.a
})
const doubled = computed(() => ror.value.a * 2)
let seen = 0
effect(() => {
  seen = doubled.value
})
r.value.a = 2
trace('before', dummy, effectRuns, doubled.value, seen)
triggerRef(ror)
trace('after', dummy, effectRuns, doubled.value, seen)
