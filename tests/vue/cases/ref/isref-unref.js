// @tier 1
// @covers V07
// isRef and unref: unref unwraps exactly one ref level; plain values, objects and nested refs pass through unchanged.
import {computed, isRef, ref, unref} from 'vue'
const r = ref(4)
const c = computed(() => r.value * 2)
const holder = {inner: ref(9)}
trace('is', isRef(r), isRef(c), isRef(4), isRef(holder), isRef(holder.inner))
trace('unref', unref(r), unref(c), unref(5), unref(holder) === holder, unref(holder.inner))
