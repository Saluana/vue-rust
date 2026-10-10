// Minimal vitest shim for the upstream unit tests (tooling/harness/run-upstream.mjs). The harness compiles it with the same
// compiler as the specs (lang js, target test), so it stays inside the language profile: no classes, no regular
// expressions, no `arguments`, no generators. It also runs in Node unchanged, which the harness unit tests rely on.
//
// Each spec is one module. Its top-level code calls describe/it/test/beforeEach at evaluation time, which only registers
// the tests. The module's exported run() (added by the harness) calls __upstreamRunAll, which runs every registered test
// in order, each with its beforeEach hooks, and reports one trace entry per test:
//   trace('upstream-test', id, status, reason)   status: pass | fail | compile-error | unsupported
// A test that the harness has refused (a construct the compiler rejects, a name the Rust vue module does not export) is a
// callback that throws __upstreamRefuse(status, reason); the runner reports that status. Assertions throw { upstream:
// 'fail', reason }, and a test that needs a dev-only behaviour throws { upstream: 'unsupported', reason }. Any other throw
// is a fail whose reason is the error's name and message.

const root = { name: null, parent: null, befores: [], afters: [] };
let current = root;
const tests = [];
const idCounts = new Map();

function fullNameOf(scope, name) {
  const parts = [];
  let s = scope;
  while (s !== null && s.name !== null) {
    parts.unshift(s.name);
    s = s.parent;
  }
  parts.push(name);
  return parts.join(' > ');
}

function registerTest(name, fn, scope) {
  const full = fullNameOf(scope, name);
  const n = idCounts.has(full) ? idCounts.get(full) + 1 : 1;
  idCounts.set(full, n);
  const id = n === 1 ? full : `${full} #${n}`;
  tests.push({ id, scope, fn });
}

export function __upstreamRefuse(status, reason) {
  throw { upstream: status, reason };
}

function refusalFn(status, reason) {
  return function () {
    throw { upstream: status, reason };
  };
}

function groupWith(name, fn) {
  const scope = { name: String(name), parent: current, befores: [], afters: [] };
  const outer = current;
  current = scope;
  let failed = false;
  let failure;
  try {
    fn();
  } catch (e) {
    failed = true;
    failure = e;
  }
  current = outer;
  if (failed) {
    registerTest('(describe callback threw)', function () {
      throw failure;
    }, outer);
  }
}

export function describe(name, fn) {
  groupWith(name, fn);
}

describe.skip = function (name, fn) {
  groupWith(name, fn);
};

describe.only = function (name, fn) {
  groupWith(name, fn);
};

describe.skipIf = function (cond) {
  return function (name, fn) {
    if (cond) {
      registerTest(name, refusalFn('unsupported', 'describe.skipIf: the condition holds in this runtime'), current);
      return;
    }
    groupWith(name, fn);
  };
};

export function test(name, fn) {
  registerTest(name, fn, current);
}

export const it = test;

test.skip = function (name, fn) {
  registerTest(name, fn, current);
};

test.only = function (name, fn) {
  registerTest(name, fn, current);
};

test.todo = function (name) {
  registerTest(name, refusalFn('unsupported', 'test.todo: no body to run'), current);
};

test.skipIf = function (cond) {
  return function (name, fn) {
    if (cond) {
      registerTest(name, refusalFn('unsupported', 'test.skipIf: the condition holds in this runtime'), current);
      return;
    }
    registerTest(name, fn, current);
  };
};

// Formats a test.each name: %s %d %i %f %j %o %O consume the row's values in order; %% is a percent sign.
function formatName(name, args) {
  let out = '';
  let k = 0;
  let i = 0;
  while (i < name.length) {
    const ch = name.charAt(i);
    if (ch === '%' && i + 1 < name.length) {
      const c = name.charAt(i + 1);
      if (c === '%') {
        out = out + '%';
        i = i + 2;
        continue;
      }
      if ('sdifjoO'.indexOf(c) !== -1 && k < args.length) {
        out = out + String(args[k]);
        k = k + 1;
        i = i + 2;
        continue;
      }
    }
    out = out + ch;
    i = i + 1;
  }
  return out;
}

test.each = function (table) {
  return function (name, fn) {
    for (let i = 0; i < table.length; i++) {
      const row = table[i];
      const args = Array.isArray(row) ? row : [row];
      registerTest(formatName(String(name), args), function () {
        return fn(...args);
      }, current);
    }
  };
};

export function beforeEach(fn) {
  current.befores.push(fn);
}

export function afterEach(fn) {
  current.afters.push(fn);
}

// Hooks that apply to a test: the root's first, then each enclosing describe's, in registration order.
function chainOf(scope) {
  const chain = [];
  let s = scope;
  while (s !== null) {
    chain.unshift(s);
    s = s.parent;
  }
  return chain;
}

function hooksOf(scope, key) {
  const out = [];
  const chain = chainOf(scope);
  for (let i = 0; i < chain.length; i++) {
    const list = key === 'before' ? chain[i].befores : chain[i].afters;
    for (let j = 0; j < list.length; j++) out.push(list[j]);
  }
  return out;
}

function truncate(text) {
  return text.length > 400 ? text.slice(0, 400) + '...' : text;
}

function verdictOf(e) {
  if (e !== null && typeof e === 'object' && typeof e.upstream === 'string') {
    return { status: e.upstream, reason: truncate(String(e.reason)) };
  }
  if (e !== null && typeof e === 'object' && typeof e.message === 'string') {
    return { status: 'fail', reason: truncate(String(e.name) + ': ' + e.message) };
  }
  return { status: 'fail', reason: truncate('thrown ' + String(e)) };
}

async function runOne(t) {
  trace('upstream-start', t.id);
  let status = 'pass';
  let reason = '';
  try {
    const befores = hooksOf(t.scope, 'before');
    for (let i = 0; i < befores.length; i++) {
      await befores[i]();
    }
    const result = t.fn();
    if (result !== undefined && result !== null && typeof result.then === 'function') {
      await result;
    }
  } catch (e) {
    const verdict = verdictOf(e);
    status = verdict.status;
    reason = verdict.reason;
  }
  try {
    const afters = hooksOf(t.scope, 'after');
    for (let i = 0; i < afters.length; i++) {
      await afters[i]();
    }
  } catch (e) {
    if (status === 'pass') {
      const verdict = verdictOf(e);
      status = verdict.status;
      reason = 'afterEach: ' + verdict.reason;
    }
  }
  trace('upstream-test', t.id, status, reason);
}

// The plan comes first, so a panic or a missing result can be attributed to a test by the harness.
export async function __upstreamRunAll() {
  for (let i = 0; i < tests.length; i++) {
    trace('upstream-plan', tests[i].id);
  }
  for (let i = 0; i < tests.length; i++) {
    await runOne(tests[i]);
  }
}

// ---- expect ----

function show(v) {
  if (typeof v === 'string') return JSON.stringify(v);
  if (typeof v === 'function') return '[function]';
  if (v === null || typeof v !== 'object') return String(v);
  try {
    return JSON.stringify(v);
  } catch (e) {
    return '[object]';
  }
}

function check(negated, pass, message) {
  if (pass === negated) {
    throw { upstream: 'fail', reason: negated ? 'expected not: ' + message : message };
  }
}

function errorMessage(err) {
  if (err !== null && typeof err === 'object' && typeof err.message === 'string') return err.message;
  return String(err);
}

// Same-value-zero-free equality used by toEqual and toStrictEqual: primitives by Object.is, arrays and plain objects
// element by element, Maps and Sets by their entries. toEqual ignores properties whose value is undefined.
function equalValues(a, b, strict) {
  if (Object.is(a, b)) return true;
  if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object') return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (strict && Object.getPrototypeOf(a) !== Object.getPrototypeOf(b)) return false;
  if (Array.isArray(a)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (!equalValues(a[i], b[i], strict)) return false;
    }
    return true;
  }
  if (a instanceof Map || a instanceof Set) {
    if (!(b instanceof Map || b instanceof Set) || a.size !== b.size) return false;
    let same = true;
    if (a instanceof Map) {
      a.forEach(function (v, k) {
        if (!b.has(k) || !equalValues(v, b.get(k), strict)) same = false;
      });
    } else {
      a.forEach(function (v) {
        if (!b.has(v)) same = false;
      });
    }
    return same;
  }
  const ka = definedKeys(a, strict);
  const kb = definedKeys(b, strict);
  if (ka.length !== kb.length) return false;
  for (let i = 0; i < ka.length; i++) {
    const k = ka[i];
    if (!hasKey(b, k)) return false;
    if (!equalValues(a[k], b[k], strict)) return false;
  }
  return true;
}

function hasKey(o, k) {
  return Object.keys(o).indexOf(k) !== -1;
}

function definedKeys(o, strict) {
  const keys = Object.keys(o);
  if (strict) return keys;
  const out = [];
  for (let i = 0; i < keys.length; i++) {
    if (o[keys[i]] !== undefined) out.push(keys[i]);
  }
  return out;
}

// Subset match for toMatchObject: every property of the expected object must match the actual value.
function matchesSubset(a, e) {
  if (Object.is(a, e)) return true;
  if (e === null || typeof e !== 'object') return equalValues(a, e, false);
  if (a === null || typeof a !== 'object') return false;
  if (Array.isArray(e)) {
    if (!Array.isArray(a) || a.length !== e.length) return false;
    for (let i = 0; i < e.length; i++) {
      if (!matchesSubset(a[i], e[i])) return false;
    }
    return true;
  }
  const keys = Object.keys(e);
  for (let i = 0; i < keys.length; i++) {
    const k = keys[i];
    if (!hasKey(a, k) || !matchesSubset(a[k], e[k])) return false;
  }
  return true;
}

function mockOf(actual) {
  if (actual === null || typeof actual !== 'function' || actual.mock === undefined) {
    throw { upstream: 'fail', reason: 'received value must be a mock or spy function' };
  }
  return actual.mock;
}

function makeMatchers(actual, negated) {
  const not = negated ? '' : 'not ';
  const m = {
    toBe: function (expected) {
      check(negated, Object.is(actual, expected), 'expected ' + show(actual) + ' to be ' + show(expected));
    },
    toEqual: function (expected) {
      check(negated, equalValues(actual, expected, false), 'expected ' + show(actual) + ' to equal ' + show(expected));
    },
    toStrictEqual: function (expected) {
      check(negated, equalValues(actual, expected, true), 'expected ' + show(actual) + ' to strictly equal ' + show(expected));
    },
    toMatchObject: function (expected) {
      check(negated, matchesSubset(actual, expected), 'expected ' + show(actual) + ' to match object ' + show(expected));
    },
    toBeDefined: function () {
      check(negated, actual !== undefined, 'expected a defined value');
    },
    toBeUndefined: function () {
      check(negated, actual === undefined, 'expected ' + show(actual) + ' to be undefined');
    },
    toBeNull: function () {
      check(negated, actual === null, 'expected ' + show(actual) + ' to be null');
    },
    toBeTruthy: function () {
      check(negated, !!actual, 'expected ' + show(actual) + ' to be truthy');
    },
    toBeFalsy: function () {
      check(negated, !actual, 'expected ' + show(actual) + ' to be falsy');
    },
    toBeInstanceOf: function (C) {
      check(negated, actual instanceof C, 'expected ' + show(actual) + ' to be an instance of the given class');
    },
    toBeLessThan: function (n) {
      check(negated, actual < n, 'expected ' + show(actual) + ' to be less than ' + show(n));
    },
    toBeGreaterThan: function (n) {
      check(negated, actual > n, 'expected ' + show(actual) + ' to be greater than ' + show(n));
    },
    toHaveLength: function (n) {
      check(negated, actual !== null && actual !== undefined && actual.length === n, 'expected length ' + show(n) + ', received ' + show(actual));
    },
    toContain: function (item) {
      const found = Array.isArray(actual) ? actual.indexOf(item) !== -1 : String(actual).indexOf(String(item)) !== -1;
      check(negated, found, 'expected ' + show(actual) + ' to contain ' + show(item));
    },
    toThrow: function (expected) {
      let threw = false;
      let err;
      try {
        actual();
      } catch (e) {
        threw = true;
        err = e;
      }
      let pass = threw;
      if (threw && expected !== undefined) {
        if (typeof expected === 'function') pass = err instanceof expected;
        else if (typeof expected === 'string') pass = errorMessage(err).indexOf(expected) !== -1;
        else pass = equalValues(errorMessage(err), expected, false);
      }
      check(negated, pass, 'expected the function to throw' + (expected === undefined ? '' : ' ' + show(expected)));
    },
    toHaveBeenCalled: function () {
      check(negated, mockOf(actual).calls.length > 0, 'expected the mock to have been called');
    },
    toHaveBeenCalledTimes: function (n) {
      const count = mockOf(actual).calls.length;
      check(negated, count === n, 'expected ' + show(n) + ' calls, received ' + show(count));
    },
    toHaveBeenCalledWith: function (...expected) {
      const calls = mockOf(actual).calls;
      let found = false;
      for (let i = 0; i < calls.length; i++) {
        if (equalValues(calls[i], expected, false)) found = true;
      }
      check(negated, found, 'expected a call with ' + show(expected));
    },
    toHaveBeenLastCalledWith: function (...expected) {
      const calls = mockOf(actual).calls;
      const last = calls.length > 0 ? calls[calls.length - 1] : undefined;
      check(negated, last !== undefined && equalValues(last, expected, false), 'expected the last call with ' + show(expected));
    },
    // Vue's dev-only console warnings (scripts/setup-vitest.ts) do not exist in the production port.
    toHaveBeenWarned: function () {
      throw { upstream: 'unsupported', reason: 'toHaveBeenWarned: dev-only warning (the production port emits none)' };
    },
    toHaveBeenWarnedLast: function () {
      throw { upstream: 'unsupported', reason: 'toHaveBeenWarnedLast: dev-only warning (the production port emits none)' };
    },
    toHaveBeenWarnedTimes: function () {
      throw { upstream: 'unsupported', reason: 'toHaveBeenWarnedTimes: dev-only warning (the production port emits none)' };
    },
  };
  m.toBeCalledTimes = m.toHaveBeenCalledTimes;
  m.toBeCalled = m.toHaveBeenCalled;
  m.toBeCalledWith = m.toHaveBeenCalledWith;
  m.toThrowError = m.toThrow;
  if (!negated) m.not = makeMatchers(actual, true);
  return m;
}

export function expect(actual) {
  return makeMatchers(actual, false);
}

// ---- vi ----

function mockFunction(impl) {
  const info = { calls: [], results: [] };
  let implementation = impl;
  // The mock forwards `this` as vitest does: a spy on a method (vi.fn(array.includes), called on the array) sees its receiver.
  const mock = function (...args) {
    info.calls.push(args);
    if (implementation === undefined) return undefined;
    return implementation.apply(this, args);
  };
  mock.mock = info;
  mock.mockClear = function () {
    info.calls = [];
    info.results = [];
    return mock;
  };
  mock.mockReset = function () {
    info.calls = [];
    info.results = [];
    implementation = undefined;
    return mock;
  };
  mock.mockImplementation = function (f) {
    implementation = f;
    return mock;
  };
  mock.mockReturnValue = function (value) {
    implementation = function () {
      return value;
    };
    return mock;
  };
  return mock;
}

export const vi = {
  fn: mockFunction,
};
