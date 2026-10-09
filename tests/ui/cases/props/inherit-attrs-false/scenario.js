// @tier 1
// @covers U13
// With inheritAttrs false, undeclared attributes are not applied to the root; the component forwards them with $attrs.
import Inner from './Inner.vue'
export function run() {
  trace('html', testHost.mount(Inner, { placeholder: 'type', 'aria-label': 'name' }).html())
}
