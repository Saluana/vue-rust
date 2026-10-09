const a = [1]
a[3] = 'x'
trace('extend', a, a.length)
delete a[0]
trace('deleted', a, 0 in a)
