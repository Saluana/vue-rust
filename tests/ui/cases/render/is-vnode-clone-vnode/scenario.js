// @tier 1
// @covers A11
// isVNode is true for a VNode and for an object whose __v_isVNode is true, and false for other values. cloneVNode copies a VNode, merges extra props with the mergeProps rules, and keeps the key.
import {cloneVNode, h, isVNode} from 'vue'
const v = h('span', {id: 'a', class: 'x', key: 'k1'}, 'hi')
trace('is-vnode', isVNode(v), isVNode({}), isVNode('span'), isVNode(null), isVNode({__v_isVNode: true}))
const c = cloneVNode(v, {id: 'b', title: 't', class: 'y'})
trace('clone', c === v, c.type, c.key, c.props.id, c.props.class, c.props.title, c.children)
trace('original-untouched', v.props.id, v.props.class, v.props.title)
const same = cloneVNode(v)
trace('no-extra-props', same === v, same.props === v.props, same.props.id, same.key)
