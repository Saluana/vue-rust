// @tier 1
// @covers V09
// isReactive, isReadonly, isProxy and isShallow report the wrapper mode. Plain values and refs report false.
import {isProxy, isReactive, isReadonly, isShallow, reactive, readonly, ref, shallowReactive, shallowReadonly} from 'vue'
const base = {n: 1}
const flags = (v) => [isReactive(v), isReadonly(v), isProxy(v), isShallow(v)]
trace('plain', flags(base))
trace('reactive', flags(reactive(base)))
trace('readonly', flags(readonly(base)))
trace('shallowReactive', flags(shallowReactive({n: 1})))
trace('shallowReadonly', flags(shallowReadonly({n: 1})))
trace('ref', flags(ref(1)))
