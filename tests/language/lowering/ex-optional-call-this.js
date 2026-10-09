const o = { name: 'obj', m() { return this.name } }
trace('opt-this', o.m?.(), o?.m(), o.nope?.(), (o?.m)())
