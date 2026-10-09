// @tier 2
// @covers L07
// Loose equality converts operands; strict equality never does. Each trace lists its pairs in order.
trace('eq-1', 1 == '1', 1 === '1')
trace('eq-0', 0 == '', 0 == '0', '' == '0')
trace('eq-null', null == undefined, null == 0, undefined == 0)
trace('eq-bool', true == 1, false == '', [] == false)
trace('eq-nan', NaN == NaN, NaN === NaN)
