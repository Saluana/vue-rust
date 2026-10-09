const n = 3
const name = 'vue'
trace('tmpl', `hello ${name}!`, `${n + 1} items`, `${1e21}|${-0}|${0.1 + 0.2}`)
trace('tmpl-nest', `a${`b${n}c`}d`, `multi
line`, ``)
