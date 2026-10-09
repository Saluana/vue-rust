// @tier 1
// @covers V03,V04,V09
// reactive() returns one proxy per target; wrapping a proxy again returns it; readonly() wraps a reactive proxy in its own proxy.
import {isReactive, isReadonly, reactive, readonly} from 'vue'
const raw = {n: 1}
const p = reactive(raw)
trace('cached', reactive(raw) === p, reactive(p) === p, p === raw)
const ro = readonly(p)
trace('readonly-wrap', ro === p, isReadonly(ro), isReactive(ro), readonly(p) === ro)
trace('readonly-of-raw', readonly(raw) === ro, isReactive(readonly(raw)))
