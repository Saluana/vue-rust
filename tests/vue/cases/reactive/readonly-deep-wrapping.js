// @tier 1
// @covers V04
// readonly() is deep: nested objects and arrays come back readonly, and writes into them are ignored too.
import {isReadonly, readonly} from 'vue'
const raw = {inner: {deep: 1}, list: [1]}
const ro = readonly(raw)
trace('deep-flags', isReadonly(ro.inner), isReadonly(ro.list), isReadonly(raw.inner))
ro.inner.deep = 2
trace('deep-value', ro.inner.deep, raw.inner.deep)
