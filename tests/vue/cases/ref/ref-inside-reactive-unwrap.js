// @tier 1
// @covers V01,V03
// A ref stored as a property of a reactive object is unwrapped on read and written through on set.
// Refs inside a reactive array or a reactive Map are not unwrapped.
import {isRef, reactive, ref} from 'vue'
const count = ref(1)
const o = reactive({count})
trace('unwrapped', o.count, isRef(o.count))
o.count = 5
trace('written-through', count.value, o.count)
const arr = reactive([ref(7)])
trace('array-element', isRef(arr[0]), arr[0].value)
const m = reactive(new Map([['k', ref(8)]]))
trace('map-element', isRef(m.get('k')), m.get('k').value)
