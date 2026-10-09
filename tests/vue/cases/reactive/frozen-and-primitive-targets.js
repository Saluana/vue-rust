// @tier 1
// @covers V03
// reactive() returns a frozen object and a primitive unchanged. Only extensible objects get a proxy.
import {reactive} from 'vue'
const frozen = Object.freeze({n: 1})
trace('frozen', reactive(frozen) === frozen)
trace('primitive', reactive(5), reactive('s'), reactive(null))
