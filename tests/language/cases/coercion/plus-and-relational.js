// @tier 2
// @covers L07
// + concatenates when either operand is a string; the other arithmetic and relational operators convert to numbers.
trace('plus', 1 + '2', '3' + 4, 1 + 2 + '3', '1' + 2 + 3)
trace('minus', '5' - 2, '5' * '2', 'a' - 1, null + 1, undefined + 1)
trace('compare', '10' < '9', 10 < '9', null >= 0, undefined < 1)
trace('empty', [] + [], [1] + [2], {} + '')
