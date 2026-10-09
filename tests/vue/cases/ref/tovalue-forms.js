// @tier 1
// @covers V08
// toValue normalises a plain value, a ref or a getter; an effect that reads the getter form tracks what the getter reads.
import {ref, toValue, watchSyncEffect} from 'vue'
const r = ref(2)
const getter = () => r.value * 10
trace('forms', toValue(7), toValue(r), toValue(getter))
const seen = []
watchSyncEffect(() => seen.push(toValue(getter)))
r.value = 3
trace('tracked', seen)
