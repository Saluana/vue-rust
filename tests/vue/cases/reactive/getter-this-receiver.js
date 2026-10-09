// @tier 1
// @covers V03
// A getter on a reactive object runs with the proxy as this, so the names it reads are tracked. A setter writes through it.
// Vue also triggers the accessor's own key after its setter returns, so an effect that reads `full` reruns once more.
import {reactive, watchSyncEffect} from 'vue'
const user = reactive({
  first: 'Ada',
  last: 'Lovelace',
  get full() {
    return `${this.first} ${this.last}`
  },
  set full(parts) {
    this.first = parts[0]
    this.last = parts[1]
  },
})
const seen = []
watchSyncEffect(() => seen.push(user.full))
user.first = 'Grace'
user.full = ['Grace', 'Hopper']
trace('seen', seen, user.last)
