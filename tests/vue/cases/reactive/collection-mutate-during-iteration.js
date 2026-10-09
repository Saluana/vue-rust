// @tier 1
// @covers V03
// Iteration visits entries added during the loop, and entries added after the current position, as native iteration does.
import {reactive} from 'vue'
const m = reactive(new Map([[1, 'a']]))
const visited = []
for (const [k] of m) {
  visited.push(k)
  if (k < 3) m.set(k + 1, 'x')
}
trace('map', visited, m.size)
const s = reactive(new Set([1]))
const seen = []
for (const v of s) {
  seen.push(v)
  if (v < 3) {
    s.delete(v)
    s.add(v + 1)
  }
}
trace('set', seen, [...s])
