let reads = 0
const o = {
  get v() {
    reads += 1
    return 'v'
  },
}
const x = o.v ?? o.v
const y = o.v || 'no'
trace('getter-calls', x, y, reads)
