// @tier 1
// @covers A10,A11,U11,U12
// h passes a props object and a slots object to a <script setup> child: the default and named slots render inside the child's template, and a child without slots renders its own empty slots.
import Wrap from './Wrap.vue'
export function run() {
  const m = testHost.mount(Wrap)
  trace('html', m.html())
  trace('children', testHost.findAll(m, 'p').length, testHost.find(m, 'span').text(), testHost.find(m, 'b').text())
}
