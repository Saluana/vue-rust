const obj = {
  v: 'o',
  outer() {
    return [1].length ? this.inner() : 'none'
  },
  inner() {
    const f = () => {
      const g = () => this.v
      return g()
    }
    return f()
  },
}
trace('arrow-nest', obj.outer())
