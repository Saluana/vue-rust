// @tier 1
// @covers V08
// toRef(source, key, default) falls back to the default while the property is undefined; toRef(getter) is read-only.
import {reactive, toRef} from 'vue'
const o = reactive({a: 1})
const missing = toRef(o, 'nope', 'fallback')
const fromGetter = toRef(() => o.a + 10)
trace('default', missing.value, fromGetter.value)
o.a = 2
trace('getter-updated', fromGetter.value)
try {
  fromGetter.value = 1
  trace('assign-ok', fromGetter.value)
} catch (e) {
  trace('assign-threw', e instanceof TypeError)
}
