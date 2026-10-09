// @tier 1
// @covers V04
// readonly() ignores writes and deletes silently in production. The original object stays writable and its changes show through.
import {readonly} from 'vue'
const raw = {n: 1}
const ro = readonly(raw)
ro.n = 2
delete ro.n
trace('after-writes', ro.n, 'n' in ro, raw.n)
raw.n = 3
trace('raw-visible', ro.n)
