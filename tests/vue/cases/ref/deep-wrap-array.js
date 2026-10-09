// @tier 1
// @covers V01
// ref() wraps an array: a push through .value reruns an effect that reads the length.
import {isReactive, ref, watchSyncEffect} from 'vue'
const list = ref([1, 2])
const lengths = []
watchSyncEffect(() => lengths.push(list.value.length))
list.value.push(3)
trace('lengths', lengths, isReactive(list.value), list.value[2])
