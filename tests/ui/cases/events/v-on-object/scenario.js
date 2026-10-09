// @tier 2
// @covers U05
// v-on with an object binds each key as an event listener.
import Obj from './Obj.vue'
export function run() {
  const m = testHost.mount(Obj, { handlers: { click: () => trace('click'), focus: () => trace('focus') } })
  testHost.click(testHost.find(m, '#o'))
  testHost.focus(testHost.find(m, '#o'))
}
