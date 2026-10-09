// @tier 2
// @covers U16,U17
// Without keys, v-for patches elements in place: removing the last item keeps the first element and its typed value.
import Plain from './Plain.vue'
export async function run() {
  const m = testHost.mount(Plain, { n: 3 })
  const first = testHost.find(m, '#f1')
  testHost.input(first, 'typed')
  await testHost.tick()
  m.setProps({ n: 2 })
  await testHost.tick()
  trace('html', m.html())
  trace('first-value', testHost.find(m, '#f1').prop('value'), testHost.findAll(m, 'input').length)
}
