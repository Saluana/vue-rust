export async function run() {
const x = 10
function f(a = () => x) { var x = 1; return a() }
trace('outer', f())
function copy(a = 3, read = () => a) { var a; a = 4; return [a, read()] }
trace('copy', copy())
function hoist(a = () => x) { function x() { return 2 }; return a() }
trace('hoist', hoist())
async function asyncCopy(a = 3, read = () => a) { var a; await 0; a = 4; return [a, read()] }
trace('async-copy', await asyncCopy())
}
