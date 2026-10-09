let out = ''
for (let i = 0; i < 4; i++) {
  try {
    if (i === 1) continue
    if (i === 3) break
    out += `t${i};`
  } finally {
    out += `f${i};`
  }
}
trace('loop-jumps', out)
function find(limit) {
  for (let i = 0; ; i++) {
    try {
      if (i === limit) return `found ${i}`
    } finally {
      out2 += 1
    }
  }
}
let out2 = 0
trace('find', find(2), out2)
