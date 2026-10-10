// @tier 1
// @covers A11
// h follows runtime-core's argument rules: three or more children are collected into an array, one VNode child is wrapped in an array, an array child is kept, and a plain object in second position is props.
import {h} from 'vue'
const a = h('i', null, 'a')
const b = h('b', null, 'b')
const list = h('ul', null, a, b, 'c')
trace('variadic', list.children.length, list.children[0] === a, list.children[1] === b, list.children[2], list.shapeFlag)
const single = h('p', null, a)
trace('single-vnode', Array.isArray(single.children), single.children.length, single.children[0] === a)
const array = h('p', null, [a, b])
trace('array-child', Array.isArray(array.children), array.children.length, array.children[1] === b)
const vnodeSecond = h('p', a)
trace('vnode-as-second', Array.isArray(vnodeSecond.children), vnodeSecond.children[0] === a, vnodeSecond.props)
const text = h('p', 'plain')
trace('string-child', text.children, text.shapeFlag, text.props)
const withProps = h('p', {id: 'x'}, 'text')
trace('props-then-text', withProps.props.id, withProps.children, withProps.shapeFlag)
const empty = h('p')
trace('no-arguments', empty.children, empty.props, empty.shapeFlag)
