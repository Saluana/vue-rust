// @tier 2
// @covers A10,A11,V01
// The render function reads a ref, so each click in an h onClick handler runs the handler, marks the component dirty and re-renders the button after the scheduler flushes.
import Counter from './Counter.vue'
export async function run() {
  const m = testHost.mount(Counter)
  trace('start', m.html())
  testHost.click(testHost.find(m, '#inc'))
  await testHost.tick()
  trace('after-one-click', m.html())
  testHost.click(testHost.find(m, '#inc'))
  testHost.click(testHost.find(m, '#inc'))
  await testHost.tick()
  trace('after-three-clicks', m.html())
}
