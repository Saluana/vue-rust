// @tier 1
// @covers A11
// h builds a tree of VNodes without rendering it: nested VNodes keep their identity, class and style are normalized, listeners stay the functions that were passed in, and shapeFlag reflects the children.
import {h} from 'vue'
let clicks = 0
const onClick = () => {
  clicks += 1
}
const inner = h('span', {id: 'a'}, 'one')
const tree = h('div', {class: ['wrap', {on: true, off: false}], style: {color: 'red'}}, [inner, h('button', {onClick, disabled: false}, 'two')])
trace('root', tree.type, tree.props.class, tree.props.style.color, tree.children.length, tree.children[0] === inner)
const button = tree.children[1]
trace('button', button.type, button.props.disabled, button.props.onClick === onClick, button.children)
button.props.onClick()
trace('after-listener-call', clicks)
trace('shape-flags', tree.shapeFlag, inner.shapeFlag, button.shapeFlag)
