// @tier 1
// @covers V04
// A readonly view of a reactive object tracks the reads it makes, so an effect over the view reruns when the source is written.
import {reactive, readonly, watchSyncEffect} from 'vue'
const src = reactive({n: 1})
const view = readonly(src)
const seen = []
watchSyncEffect(() => seen.push(view.n))
src.n = 2
view.n = 9
trace('seen', seen, src.n)
