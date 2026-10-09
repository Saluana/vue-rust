const o = { b: 1, 10: 'ten', a: 2, 2: 'two' }
o.c = 3
let keys = ''
for (const k in o) keys += `${k},`
trace('keys', keys)
let seen = ''
const p = { x: 1, y: 2, z: 3 }
for (const k in p) {
  seen += k
  if (k === 'x') delete p.y
}
trace('deleted-skipped', seen)
let none = ''
for (const k in null) none += 'never'
for (const k in undefined) none += 'never'
trace('nullish-in', none === '')
let sh = ''
for (const k in 'ab') sh += k
trace('string-keys', sh)
