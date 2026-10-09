const f0 = () => 0
const f1 = (a) => a
const f2 = function (a, b) {}
const f3 = (a, b = 2, c) => a
const f4 = (...rest) => rest
const f5 = ({ x }, [y]) => x
trace('lengths', f0.length, f1.length, f2.length, f3.length, f4.length, f5.length)
