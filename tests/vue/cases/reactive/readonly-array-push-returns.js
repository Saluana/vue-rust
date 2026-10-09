// @tier 1
// @covers V04
// A readonly array ignores push: no element is stored, but push still returns the length it would have reached.
import {readonly} from 'vue'
const ro = readonly([1, 2, 3])
const returned = ro.push(4)
trace('push', returned, ro.length, ro[3])
