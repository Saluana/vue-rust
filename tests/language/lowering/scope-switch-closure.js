const fs = [0, 0]
let idx = 0
switch (1) {
  case 1: {
    const v = 'one'
    fs[idx] = () => v
    idx += 1
  }
  case 2: {
    const v = 'two'
    fs[idx] = () => v
    idx += 1
  }
}
trace('switch-capture', fs[0](), fs[1]())
