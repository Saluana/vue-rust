// @tier 2
// @covers V24,V01
// A provided ref is shared, not copied: the injector's write is seen by the provider's template.
import Provider from './Provider.vue'
export async function run() {
  const m = testHost.mount(Provider)
  testHost.click(testHost.find(m, '#b'))
  testHost.click(testHost.find(m, '#b'))
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text())
}
