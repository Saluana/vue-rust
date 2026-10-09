// @tier 1
// @covers V07
// proxyRefs unwraps top-level ref keys on read. A write to such a key sets the ref's value; a write to a plain key replaces that value.
import {proxyRefs, ref} from 'vue'
const count = ref(1)
const label = ref('a')
const p = proxyRefs({count, label, plain: 3})
trace('read', p.count, p.label, p.plain)
p.count = 5
trace('after-ref-key-write', count.value, p.count)
p.label = 'b'
trace('after-label-write', label.value, p.label)
p.plain = 4
trace('after-plain-write', p.plain)
count.value = 9
trace('after-direct-ref-write', p.count)
