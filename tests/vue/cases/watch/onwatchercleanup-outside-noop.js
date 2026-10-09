// @tier 1
// @covers V16
// Outside a watcher, onWatcherCleanup registers nothing, returns nothing, and does not throw.
import {onWatcherCleanup} from 'vue'
let ran = false
const result = onWatcherCleanup(() => {
  ran = true
})
trace('outside', result, ran)
