// @tier 1
// @covers U13,U30
// A component with two roots gets no automatic attribute fallthrough: the class passed to it is not applied to either root.
import Multi from './Multi.vue'
export function run() {
  trace('html', testHost.mount(Multi, { class: 'x', title: 't' }).html())
}
