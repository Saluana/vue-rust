// @tier 1
// @covers V09,V10
// markRaw keeps an object unwrapped: reactive() returns it as is, and a read through a reactive parent gives the raw object.
import {isProxy, isReactive, markRaw, reactive, toRaw} from 'vue'
const inner = markRaw({n: 1})
const outer = reactive({inner})
trace('raw', outer.inner === inner, isReactive(outer.inner), isProxy(outer.inner), toRaw(inner) === inner)
trace('reactive-returns-same', reactive(inner) === inner)
