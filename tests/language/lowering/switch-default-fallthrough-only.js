let s = ''
switch ('zzz') {
  default:
    s += 'd;'
  case 'a':
    s += 'a;'
}
trace('default-first', s)
