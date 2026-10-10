// @tier 2
// @covers A10,A11,V01
// The same setup form, returning a render function, works in a .vue default export, a .js module and a .ts module. The three render in one tree, and the .js and .ts components re-render on their own clicks.
import App from './App.vue'
export async function run() {
  const m = testHost.mount(App)
  trace('start', m.html())
  testHost.click(testHost.find(m, '#js'))
  testHost.click(testHost.find(m, '#ts'))
  testHost.click(testHost.find(m, '#ts'))
  await testHost.tick()
  trace('after-clicks', m.html())
}
