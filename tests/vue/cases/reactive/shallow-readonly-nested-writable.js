// @tier 1
// @covers V05
// shallowReadonly guards only the top level: a root write is ignored, and a nested object stays writable.
import {isReadonly, shallowReadonly} from 'vue'
const inner = {n: 1}
const s = shallowReadonly({top: 1, inner})
s.top = 2
trace('top-write-ignored', s.top, isReadonly(s.top))
s.inner.n = 5
trace('nested-write-applied', inner.n, isReadonly(s.inner))
