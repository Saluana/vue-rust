// @tier 1
// @covers U13
// Attributes that a component does not declare fall through to its single root element, including class, id and data attributes.
import Fall from './Fall.vue'
export function run() {
  trace('html', testHost.mount(Fall, { class: 'p', 'data-q': '1', title: 't', id: 'fx' }).html())
}
