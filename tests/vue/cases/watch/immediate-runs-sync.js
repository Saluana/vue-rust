// @tier 1
// @covers V14
// immediate: true calls back during watch() itself, with the current value and no old value.
import {ref, watch} from 'vue'
const n = ref(7)
const log = []
const stop = watch(n, (v, old) => log.push([v, old === undefined]), {immediate: true})
trace('sync-after-watch', log)
stop()
