// @tier 2
// @covers A10,A11,U05,U14
// defineComponent(setupFunction) is the function form: the function is the setup, and the component's render is what it returns. It works in <script setup>, as a default export (Fn.vue), and with an extra options argument (inheritAttrs false keeps class off the root). A v-if toggles one such component.
import App from './App.vue'
export async function run() {
  const m = testHost.mount(App)
  trace('start', m.html())
  testHost.click(testHost.find(m, '#toggle'))
  await testHost.tick()
  trace('hidden', m.html())
  testHost.click(testHost.find(m, '#toggle'))
  await testHost.tick()
  trace('shown-again', m.html())
}
