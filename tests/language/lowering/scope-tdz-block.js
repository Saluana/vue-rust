let x = 'outer'
try {
  {
    trace('before-inner', x)
    let x = 'inner'
    trace('never', x)
  }
} catch (e) {
  trace('tdz-block', e.name, x)
}
