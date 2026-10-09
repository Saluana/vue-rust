const o = {
  parts: 'a',
  add(s) {
    this.parts += s
    return this
  },
}
trace('chain', o.add('b').add('c').parts)
