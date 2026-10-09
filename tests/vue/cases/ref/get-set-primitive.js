// @tier 1
// @covers V01,V07
// A ref keeps its value in .value. An effect that reads .value reruns on each write; isRef tells refs from plain values.
import {isRef, ref, watchSyncEffect} from 'vue'
const count = ref(1)
const seen = []
watchSyncEffect(() => seen.push(count.value))
count.value = 2
count.value = 3
trace('values', seen, count.value, isRef(count), isRef(count.value))
