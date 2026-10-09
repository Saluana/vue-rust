// @tier 1
// @covers V07
// Only top-level keys unwrap. A ref inside a nested plain object is returned as the ref itself, not unwrapped.
import {isRef, proxyRefs, ref} from 'vue'
const inner = ref(7)
const p = proxyRefs({nested: {deep: inner}, top: ref(1)})
trace('top-unwrapped', p.top)
trace('nested', isRef(p.nested.deep), p.nested.deep === inner, p.nested.deep.value)
