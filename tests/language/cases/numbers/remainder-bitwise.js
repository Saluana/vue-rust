// @tier 2
// @covers L08
// % keeps the sign of the dividend; bitwise operators convert operands to 32-bit integers.
trace('rem', 5 % -3, -5 % 3, 5.5 % 2, -4 % 2)
trace('bits', 5 & 3, 5 | 3, 5 ^ 3, ~5, 1 << 31, -1 >>> 0, -1 >> 1)
trace('convert', ~~3.7, 2 ** 32 | 0, (2 ** 31) | 0, 4294967295 >>> 0)
