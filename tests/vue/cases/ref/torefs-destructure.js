// @tier 1
// @covers V08
// toRefs turns each property of a reactive object into a linked ref; the key set is unchanged.
import {isRef, reactive, toRefs, watchSyncEffect} from 'vue'
const state = reactive({x: 1, y: 'a'})
const {x, y} = toRefs(state)
let runs = 0
watchSyncEffect(() => {
  x.value
  runs += 1
})
state.x = 2
y.value = 'b'
trace('refs', isRef(x), isRef(y), x.value, y.value, state.y, Object.keys(toRefs(state)), runs)
