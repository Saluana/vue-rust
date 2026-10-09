// @tier 1
// @covers V12,V15
// A watch created inside an effect scope stops with the scope. Writes after the scope stops reach nothing.
import {effectScope, ref, watch} from 'vue'
const n = ref(0)
const log = []
const scope = effectScope()
scope.run(() => watch(n, (v) => log.push(v), {flush: 'sync'}))
n.value = 1
scope.stop()
n.value = 2
trace('log', log)
