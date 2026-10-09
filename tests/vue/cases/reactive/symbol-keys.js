// @tier 1
// @covers V03
// A symbol-keyed property is tracked like a string key. Object.keys does not list it.
import {reactive, watchSyncEffect} from 'vue'
const key = Symbol('k')
const o = reactive({[key]: 1, n: 0})
const seen = []
watchSyncEffect(() => seen.push(o[key]))
o[key] = 2
trace('symbol', seen, o[key], Object.keys(o))
