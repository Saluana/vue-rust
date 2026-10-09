// @tier 3
// @covers U22
// With defer, the Teleport resolves its target after its component mounts; the content is in the target before mount returns.
import Page from './Page.vue'
export async function run() {
  const late = testHost.container('late')
  const m = testHost.mount(Page)
  trace('sync', m.html(), late.html())
  await testHost.tick()
  trace('flushed', late.html())
}
