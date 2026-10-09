// @tier 1
// @covers V02
// shallowRef does not wrap: a nested mutation is invisible to effects until triggerRef is called.
import {isReactive, shallowRef, triggerRef, watchSyncEffect} from 'vue'
const raw = {n: 1}
const s = shallowRef(raw)
const seen = []
watchSyncEffect(() => seen.push(s.value.n))
s.value.n = 2
const before = seen.slice()
triggerRef(s)
trace('shallow', before, seen, isReactive(s.value), s.value === raw)
