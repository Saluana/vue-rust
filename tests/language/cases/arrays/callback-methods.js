// @tier 2
// @covers L10
// Callback and copy methods on a small numeric array, with explicit expected results.
const nums = [1, 2, 3, 4, 5]
trace('map-filter', nums.map((n) => n * n).filter((n) => n % 2 === 1))
trace('reduce', nums.reduce((acc, n) => acc + n, 0), nums.reduceRight((acc, n) => acc + '' + n, ''))
trace('find', nums.find((n) => n > 2), nums.findIndex((n) => n > 10), nums.some((n) => n > 4), nums.every((n) => n > 0))
trace('flat', [1, [2, [3, [4]]]].flat(), [1, [2, [3, [4]]]].flat(2), [[], [1]].flat())
trace('concat-slice', [1].concat([2], 3, [[4]]), nums.slice(-2), nums.indexOf(3), nums.includes(NaN), [NaN].includes(NaN))
