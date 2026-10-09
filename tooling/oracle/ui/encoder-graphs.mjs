// Value graphs for the encoder-equivalence test (WP-V1): each entry is a function that builds a fresh value, so the Node
// process and the browser page encode the same graphs. Pure JavaScript; no Date, random numbers or built-in functions.
export function graphs() {
  const alias = { shared: 1 };
  const cycle = { name: 'loop' };
  cycle.self = cycle;
  const holes = [1, , 3];
  holes.extra = 'x';
  const accessor = {};
  Object.defineProperty(accessor, 'computed', { get() { return 1; }, enumerable: true });
  Object.defineProperty(accessor, 'hidden', { value: 2, enumerable: false });
  const withSymbol = { plain: 1 };
  withSymbol[Symbol('ignored')] = 2;
  const frozen = Object.freeze({ a: 1 });
  const sealed = Object.seal({ b: 2 });
  const noExt = Object.preventExtensions({ c: 3 });
  function Named(a, b) {
    this.x = a + b;
  }
  return [
    () => undefined,
    () => null,
    () => true,
    () => false,
    () => [0, -0, NaN, Infinity, -Infinity, 1e21, 0.1, -5, 2 ** 53],
    () => ['', 'a', '\ud800', 'é', ' ', 'a\u0001b', 'quote"back\\slash'],
    () => [Symbol('desc'), Symbol(), Symbol('desc')],
    () => [function foo(a, b) { return a + b; }, () => 1, function () { return 2; }],
    () => ({ b: 1, 10: 2, 2: 3, a: { deep: [1, { x: 'y' }] } }),
    () => [1, [2, [3, [4, [5]]]]],
    () => holes,
    () => accessor,
    () => withSymbol,
    () => new Map([[{ k: 1 }, 'v'], ['s', { w: 2 }]]),
    () => new Set([1, 'two', { three: 3 }, null]),
    () => new Error('plain'),
    () => new TypeError('typed'),
    () => Promise.resolve(7),
    () => ({ first: alias, second: alias }),
    () => cycle,
    () => frozen,
    () => sealed,
    () => noExt,
    () => Object.freeze([1, 2]),
    () => Object.assign(Object.create(null), { key: 'v' }),
    () => new Named(1, 2),
    () => ({ $: 'x', t: 'not a tag' }),
    () => Array.from({ length: 300 }, (_, i) => i * 3),
    () => ({ '': 'empty key', 'with space': 1, 'ünï': 2 }),
    () => [new Map(), new Set(), [], {}],
    () => ({ outer: [new Map([['deep', new Set([alias])]])] }),
  ];
}
