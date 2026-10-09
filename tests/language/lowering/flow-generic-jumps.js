let out = ''
blk: for (let i = 0; i < 3; i++) {
  try {
    switch (i) {
      case 0:
        out += 'z;'
        continue blk
      case 1:
        out += 'o;'
        break blk
    }
    out += 'never;'
  } finally {
    out += `f${i};`
  }
}
trace('generic-jumps', out)
