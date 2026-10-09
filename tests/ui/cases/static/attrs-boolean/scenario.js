// @tier 2
// @covers U03,U30
// Boolean and absent attributes: false removes boolean attributes, undefined removes any attribute, other falsy values stay strings.
import Attrs from './Attrs.vue'
export async function run() {
  const m = testHost.mount(Attrs, { off: false, title: undefined, hidden: false })
  trace('initial', m.html())
  m.setProps({ off: true, title: 'hi', hidden: true })
  await testHost.tick()
  trace('updated', m.html())
  trace('title', testHost.find(m, '#p').attr('title'), testHost.find(m, '#i').prop('disabled'))
  m.setProps({ off: false, title: '', hidden: false })
  await testHost.tick()
  trace('cleared', m.html())
}
