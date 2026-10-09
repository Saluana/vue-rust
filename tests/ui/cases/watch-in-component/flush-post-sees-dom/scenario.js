// @tier 2
// @covers V14,V17
// A post-flush watcher reads the DOM after the update; a pre-flush watcher reads the text from before it.
import Post from './Post.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Post, { n: 1, log })
  m.setProps({ n: 2, log })
  await testHost.tick()
  trace('html', m.html())
}
