// @tier 1
// @covers V07
// proxyRefs returns a reactive object unchanged, so no second unwrapping layer is added. Writes go through the reactive object.
import {proxyRefs, reactive, ref} from 'vue'
const r = reactive({count: ref(3), n: 1})
const p = proxyRefs(r)
trace('same-object', p === r)
trace('values', p.count, r.count, p.n)
p.count = 4
trace('after-write', r.count, p.count)
