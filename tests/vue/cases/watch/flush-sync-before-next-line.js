// @tier 1
// @covers V14
// flush: 'sync' calls back during the write itself, once per changing write, before the next statement runs.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, (v) => log.push(`sync:${v}`), {flush: 'sync'})
n.value = 1
trace('after-write-1', log)
n.value = 2
n.value = 2
trace('after-writes', log)
