// @tier 2
// @covers U11
// v-model on a component: the child receives modelValue and reports changes through update:modelValue.
import Form from './Form.vue'
export async function run() {
  const m = testHost.mount(Form)
  testHost.input(testHost.find(m, '#f'), 'abc')
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text(), testHost.find(m, '#f').prop('value'))
}
