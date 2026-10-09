// @tier 1
// @covers V03
// includes, indexOf and lastIndexOf find an element by identity whether the argument is the raw object or its proxy.
import {reactive, toRaw} from 'vue'
const item = {id: 1}
const list = reactive([item])
const proxied = list[0]
trace('reactive-search', list.includes(item), list.includes(proxied), list.indexOf(item), list.indexOf(proxied), list.lastIndexOf(toRaw(proxied)))
const raw = [item]
const wrapped = reactive(raw)
trace('raw-array-search', raw.includes(proxied), wrapped.includes(proxied))
