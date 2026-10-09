let kind = ''
try {
  const u = undefined
  u.prop = 1
} catch (e) {
  kind = `${e.name}:${e instanceof TypeError}`
}
trace('kind', kind)
