// WP-G0A: executable reproductions of KNOWN prototype defects.
// Every test asserts the OLD (buggy) behaviour and passes today. When a defect is fixed its test fails: flip the
// assertion to the corrected behaviour at that point. Compilers are imported from these fixture copies only.
// Run: node --test tests/fixtures/prototype/regressions.test.mjs
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {compile as counterCompile} from './counter/compiler/compile.mjs';
import {compile as quickQuoteCompile} from './quick-quote/compiler/compile.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const PROJECT = resolve(HERE, '../../..');
const ENV_SCRIPT = join(PROJECT, 'scripts/env.sh');
const COMPILERS = [['counter', counterCompile], ['quick-quote', quickQuoteCompile]];
const component = (script, template) => `<script setup>${script}</script><template>${template}</template>`;
const INC = "import {ref} from 'vue'; const count = ref(0); function inc() { count.value++ }";

// Comparison harness: a Rust output line agrees with the JavaScript oracle only when the strings are identical.
const same = (rustLine, jsText) => rustLine === jsText;

// Mirrors `source scripts/env.sh; rustc ...`, run in the project directory so rust-toolchain.toml selects 1.99.0.
function rustc(args) {
  return spawnSync('bash', ['-c', 'source "$0" && exec rustc "$@"', ENV_SCRIPT, ...args], {cwd: PROJECT, encoding: 'utf8'});
}

// Compiles a std-only probe into os.tmpdir(), runs it, removes the directory, and returns stdout lines.
function runProbe(source, label) {
  const dir = mkdtempSync(join(tmpdir(), 'vn-g0a-probe-'));
  try {
    const src = join(dir, 'probe.rs');
    const bin = join(dir, 'probe');
    writeFileSync(src, source);
    const built = rustc(['--edition', '2024', '-o', bin, src]);
    assert.equal(built.status, 0, `${label}: rustc exited ${built.status}\n${built.stderr}`);
    const ran = spawnSync(bin, [], {encoding: 'utf8'});
    assert.equal(ran.status, 0, `${label}: probe exited ${ran.status}\n${ran.stderr}`);
    return ran.stdout.replace(/\n$/, '').split('\n');
  } finally {
    rmSync(dir, {recursive: true, force: true});
  }
}

// ---- (a) dynamic event argument -------------------------------------------------------------------------------

for (const [name, compile] of COMPILERS) {
  test(`KNOWN BUG (prototype): @[click] dynamic event argument is accepted and compiled as a fixed click handler (${name} compiler)`, () => {
    const dynamic = compile(component(INC, '<button @[click]="inc">Add</button>'));
    const fixed = compile(component(INC, '<button @click="inc">Add</button>'));
    // The dynamic argument's value is never read: the output is byte-identical to the static @click handler.
    assert.equal(dynamic, fixed);
    assert.match(dynamic, /\.clicked\(\) \{ self\.inc\(\); \}/);
    // Control: a dynamic argument whose source text is not 'click' is still rejected loudly.
    assert.throws(() => compile(component(INC, '<button @[evt]="inc">Add</button>')), /Unsupported\/duplicate directive on:evt/);
    // Control: the equality above is not vacuous, because a different static handler changes the output.
    assert.notEqual(compile(component(`${INC} function reset() { count.value = 0 }`, '<button @click="reset">Add</button>')), fixed);
  });
}

// ---- (b) interpolated numbers use Rust Display ---------------------------------------------------------------

// Each row: the Rust literal used in the probe, and the same value as a JavaScript number (oracle: String(x)).
const DISPLAY = [
  {rust: '1e21', js: 1e21},
  {rust: '1e-7', js: 1e-7},
  {rust: '-0.0', js: -0},
  {rust: '0.1 + 0.2', js: 0.1 + 0.2},
  {rust: '123456789012345680000.0', js: 123456789012345680000},
  {rust: 'f64::NAN', js: NaN},
  {rust: 'f64::INFINITY', js: Infinity},
  {rust: '5e-324', js: 5e-324},
];
const displayProgram = `fn main() {\n    let xs: [f64; ${DISPLAY.length}] = [${DISPLAY.map(c => c.rust).join(', ')}];\n    for x in xs {\n        let s = format!("{}", x);\n        println!("{}", s);\n    }\n}\n`;
let displayLines;
const displayOutput = () => (displayLines ??= runProbe(displayProgram, 'Display probe'));

test('KNOWN BUG (prototype): interpolated numbers use Rust Display, which diverges from JS String(n) for some values', (t) => {
  for (const [name, compile] of COMPILERS) {
    const code = compile(component("import {ref} from 'vue'; const big = ref(1e21);", '<p>{{ big }}</p>'));
    assert.ok(code.includes('format!("{}", self.big)'), `${name} compiler: interpolation is not emitted with format!("{}")`);
  }
  const rust = displayOutput();
  const js = DISPLAY.map(c => String(c.js));
  t.diagnostic(`Rust format!("{}") outputs: ${JSON.stringify(rust)}`);
  t.diagnostic(`JS String(x) oracle:        ${JSON.stringify(js)}`);
  assert.equal(rust.length, DISPLAY.length);
  const divergent = DISPLAY.filter((c, i) => !same(rust[i], js[i])).map(c => c.rust);
  assert.deepEqual(divergent, ['1e21', '1e-7', '-0.0', 'f64::INFINITY', '5e-324']);
  // Controls: values that agree stay in agreement, and the comparison rejects a deliberately wrong expectation.
  assert.equal(same(rust[3], js[3]), true);
  assert.equal(same(rust[4], js[4]), true);
  assert.equal(same(rust[5], js[5]), true);
  assert.equal(same(rust[3], '0.3'), false);
});

// ---- (c) Quick Quote toFixed ----------------------------------------------------------------------------------

const TOFIXED = [['0.5', 0], ['2.5', 0], ['1.005', 2]];

// Returns the Rust expression the Quick Quote compiler emits for (value).toFixed(digits), read from its getter.
function emittedToFixed(value, digits) {
  const code = quickQuoteCompile(component(`import {computed} from 'vue'; const a = computed(() => (${value}).toFixed(${digits}));`, '<p>{{ a }}</p>'));
  const m = /^pub fn a\(&self\) -> String \{ (.*) \}$/m.exec(code);
  assert.ok(m, `computed a was not emitted as a Rust getter for (${value}).toFixed(${digits})`);
  return m[1];
}

test('KNOWN BUG (prototype): Quick Quote toFixed emits Rust format!("{:.N}"), which rounds ties to even: (0.5).toFixed(0) and (2.5).toFixed(0) diverge from JS', (t) => {
  const emitted = TOFIXED.map(([v, d]) => emittedToFixed(v, d));
  assert.deepEqual(emitted, ['format!("{:.0}", 0.5f64)', 'format!("{:.0}", 2.5f64)', 'format!("{:.2}", 1.005f64)']);
  const js = TOFIXED.map(([v, d]) => Number(v).toFixed(d));
  assert.equal(js.join(','), '1,3,1.00'); // V8 oracle, matching ECMA-262 Number.prototype.toFixed (ties pick the larger n)
  const program = `fn main() {\n${emitted.map(e => `    println!("{}", ${e});\n`).join('')}}\n`;
  const rust = runProbe(program, 'toFixed probe');
  t.diagnostic(`Rust outputs of the emitted expressions: ${JSON.stringify(rust)}`);
  t.diagnostic(`JS (0.5).toFixed(0), (2.5).toFixed(0), (1.005).toFixed(2): ${JSON.stringify(js)}`);
  assert.deepEqual(rust, ['0', '2', '1.00']);
  assert.equal(same(rust[0], js[0]), false);
  assert.equal(same(rust[1], js[1]), false);
  // Control: (1.005).toFixed(2) does not diverge, because 1.005 is stored just below the tie.
  assert.equal(same(rust[2], js[2]), true);
  assert.equal(same(rust[0], '1'), false);
});

// ---- (d) computed values are uncached getters ------------------------------------------------------------------

for (const [name, compile] of COMPILERS) {
  test(`KNOWN BUG (prototype): computed values are uncached Rust getters that recompute on every read (${name} compiler)`, () => {
    const code = compile(component("import {ref, computed} from 'vue'; const count = ref(0); const doubled = computed(() => count.value * 2);", '<p>{{ doubled }}</p>'));
    assert.ok(code.includes('pub fn doubled(&self) -> f64 { (self.count * 2.0f64) }'), 'computed is not an inline getter');
    assert.ok(code.includes('pub struct App { pub count: f64, }'), 'App struct holds a field besides the ref');
    assert.doesNotMatch(code, /cache/i);
    assert.ok(code.includes('format!("{}", self.doubled())'), 'template read does not call the getter');
  });
}
