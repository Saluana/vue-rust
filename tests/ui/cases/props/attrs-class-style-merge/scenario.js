// @tier 1
// @covers U13
// The parent's class and style merge with the child's root class and style rather than replacing them.
import Child from './Child.vue'
export function run() {
  trace('html', testHost.mount(Child, { class: 'p', style: 'margin: 0px' }).html())
}
