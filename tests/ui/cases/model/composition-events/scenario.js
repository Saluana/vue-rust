// @tier 2
// @covers U07
// During an IME composition v-model ignores input events; compositionend commits the value.
import Comp from './Comp.vue'
export async function run() {
  const m = testHost.mount(Comp)
  const c = testHost.find(m, '#c')
  testHost.dispatch(c, 'compositionstart')
  testHost.input(c, 'ka')
  await testHost.tick()
  trace('composing', testHost.find(m, '#out').text())
  testHost.dispatch(c, 'compositionend', { data: 'ka' })
  await testHost.tick()
  trace('committed', testHost.find(m, '#out').text())
}
