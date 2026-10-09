// @tier 1
// @covers V04,V09
// Wrappers compose. readonly(reactive(o)) is readonly and still reactive. reactive(readonly(o)) returns the readonly proxy.
import {isProxy, isReactive, isReadonly, reactive, readonly} from 'vue'
const raw = {n: 1}
const ro = readonly(reactive(raw))
trace('readonly-of-reactive', isReadonly(ro), isReactive(ro), isProxy(ro))
const rr = reactive(readonly(raw))
trace('reactive-of-readonly', isReadonly(rr), isReactive(rr), rr === readonly(raw), rr.n)
