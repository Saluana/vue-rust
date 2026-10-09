// @tier 1
// @covers U30
// The SFC compile keeps template comments in the production build, so they are comment nodes in the html and the snapshot.
import Cm from './Cm.vue'
export function run() {
  const m = testHost.mount(Cm)
  trace('html', m.html())
  trace('snapshot', m.snapshot())
}
