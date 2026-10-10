// @tier 1
// @covers U13
// mergeProps joins class and style into one value each, calls both listeners of a key in order when the element is clicked, and keeps the other keys.
import Child from './Child.vue'
export function run() {
  const log = []
  const m = testHost.mount(Child, {log})
  const button = testHost.find(m, 'button')
  trace('attrs', button.attr('class'), button.attr('style'), button.attr('title'))
  testHost.click(button)
  trace('listeners', log)
}
