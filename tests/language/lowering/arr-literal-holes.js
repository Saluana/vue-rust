const a = [1, , 3]
trace('holes', a, a.length, 1 in a, 0 in a)
trace('trailing', [1, 2, ,].length)
