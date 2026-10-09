// WP-G0A: re-checks the README and evidence/verification.txt claims that the JavaScript side can reproduce.
// Uses the byte-identical fixture copies (tests/fixtures/prototype/counter, quick-quote), so the result does not
// move when the root compiler/ is replaced. Writes only to a temporary directory, which is removed at the end.
// Run from anywhere:  node docs/evidence/baseline/readme-claims-probe.mjs
import {existsSync, mkdtempSync, readFileSync, rmSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {compile as counterCompile} from '../../../tests/fixtures/prototype/counter/compiler/compile.mjs';
import {compile as quickQuoteCompile} from '../../../tests/fixtures/prototype/quick-quote/compiler/compile.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const PROJECT = resolve(HERE, '../../..');
const FIX = join(PROJECT, 'tests/fixtures/prototype/counter');
const sha256 = buf => createHash('sha256').update(buf).digest('hex');
const component = (script, template) => `<script setup>${script}</script><template>${template}</template>`;
const STATE = "import {ref} from 'vue'; const count = ref(0);";

const out = [];
const say = (...parts) => out.push(parts.join(' '));
const lineWith = (code, pattern) => code.split('\n').filter(l => pattern.test(l)).map(l => l.trim()).join(' | ') || '(no matching line)';

// Each case reports ACCEPTED with the matching emitted line, or REJECTED with the compiler's message.
function probe(id, label, compile, source, pattern) {
  try {
    const code = compile(source);
    say(`[${id}] ${label}: ACCEPTED`, pattern ? `-> ${lineWith(code, pattern)}` : '');
  } catch (e) {
    say(`[${id}] ${label}: REJECTED -> ${e.message}`);
  }
}

say('== A. Compiler behaviour of the counter fixture compiler (README "What works" and "What does not work yet")');
probe('A1', 'elements main/section/div/h1/h2/p/span/label', counterCompile,
  component(STATE, '<main><section><div><h1>A</h1><h2>B</h2><p>p</p><span>s</span><label>l</label></div></section></main>'), /ui\.vertical/);
probe('A2', 'primitive refs number/string/boolean', counterCompile,
  component("import {ref} from 'vue'; const a = ref(1); const b = ref('x'); const c = ref(true);", '<p>{{ a }}</p>'), /pub [abc]: /);
probe('A3', 'ref assignment count = 5 in a handler', counterCompile,
  component(STATE, '<button @click="count = 5">x</button>'), /self\.count = 5/);
probe('A4', 'decrement count-- in a handler', counterCompile,
  component(STATE, '<button @click="count--">x</button>'), /-= 1\.0/);
probe('A5', ':disabled with strict comparison', counterCompile,
  component(STATE + ' function inc() { count.value++ }', '<button @click="inc" :disabled="count === 0">x</button>'), /add_enabled/);
probe('A6', 'strict !== comparison in v-if', counterCompile,
  component(STATE, '<p v-if="count !== 0">x</p>'), /if \(self\.count/);
probe('A7', 'handler with a parameter (must be zero-argument)', counterCompile,
  component(STATE + ' function f(x) { count.value++ }', '<button @click="f">x</button>'), null);
probe('A8', '<script setup lang="ts"> with plain TypeScript (no annotations)', counterCompile,
  `<script setup lang="ts">import {ref} from 'vue'; const count = ref(0);</script><template><p>{{ count }}</p></template>`, /pub count: f64/);
probe('A9', 'custom component <MyCard/> (nested components)', counterCompile,
  component(STATE, '<MyCard/>'), null);
probe('A10', '<slot/> in template', counterCompile,
  component(STATE, '<slot/>'), null);
probe('A11', 'defineProps({}) statement', counterCompile,
  component(STATE + 'defineProps({});', '<p>x</p>'), null);
probe('A12', 'defineEmits([...]) statement', counterCompile,
  component(STATE + "defineEmits(['saved']);", '<p>x</p>'), null);
probe('A13', 'reactive({}) (import from vue)', counterCompile,
  component("import {reactive} from 'vue'; const s = reactive({a: 1});", '<p>x</p>'), null);
probe('A14', 'watch(count, ...) (import from vue)', counterCompile,
  component("import {ref, watch} from 'vue'; const count = ref(0); watch(count, () => {});", '<p>x</p>'), null);
probe('A15', 'onMounted(...) lifecycle hook', counterCompile,
  component("import {onMounted} from 'vue'; onMounted(() => {});", '<p>x</p>'), null);
probe('A16', 'import from vue-router (routing)', counterCompile,
  component("import {useRouter} from 'vue-router'; const r = useRouter();", '<p>x</p>'), null);
probe('A17', 'import thing from npm package', counterCompile,
  component("import thing from 'anything';", '<p>x</p>'), null);
probe('A18', 'fetch(...) call (async/network)', counterCompile,
  component(STATE + "fetch('/x');", '<p>x</p>'), null);
probe('A19', 'console.log(...) (DOM/browser API)', counterCompile,
  component(STATE + 'console.log(count);', '<p>x</p>'), null);
probe('A20', '<style> block (CSS)', counterCompile,
  component(STATE, '<p>x</p>') + '<style>p{color:red}</style>', null);
probe('A21', 'v-for list directive', counterCompile,
  component(STATE, '<p v-for="x in count">x</p>'), null);
probe('A22', 'TypeScript generic ref<number>(0)', counterCompile,
  component("import {ref} from 'vue'; const count = ref<number>(0);", '<p>x</p>'), null);
probe('A23', 'Options API export default', counterCompile,
  '<script>export default {}</script><template><p/></template>', null);

say('');
say('== B. Dynamic event argument and interpolated number text (KNOWN BUG cases)');
probe('B1', '@[click]="inc" (counter)', counterCompile,
  component("import {ref} from 'vue'; const count = ref(0); function inc() { count.value++ }", '<button @[click]="inc">x</button>'), /clicked/);
probe('B2', '@[evt]="inc" (counter)', counterCompile,
  component("import {ref} from 'vue'; const count = ref(0); function inc() { count.value++ }", '<button @[evt]="inc">x</button>'), null);
probe('B3', '@[click]="inc" (quick-quote)', quickQuoteCompile,
  component("import {ref} from 'vue'; const count = ref(0); function inc() { count.value++ }", '<button @[click]="inc">x</button>'), /clicked/);
probe('B4', '@[evt]="inc" (quick-quote)', quickQuoteCompile,
  component("import {ref} from 'vue'; const count = ref(0); function inc() { count.value++ }", '<button @[evt]="inc">x</button>'), null);
probe('B5', 'interpolated ref(1e21) (counter)', counterCompile,
  component("import {ref} from 'vue'; const big = ref(1e21);", '<p>{{ big }}</p>'), /format!\("\{\}", self\.big\)|pub struct|big: 1e/);
probe('B6', 'toFixed (counter; quick-quote only)', counterCompile,
  component("import {computed} from 'vue'; const a = computed(() => (0.5).toFixed(0));", '<p>{{ a }}</p>'), null);
probe('B7', 'toFixed (quick-quote)', quickQuoteCompile,
  component("import {computed} from 'vue'; const a = computed(() => (0.5).toFixed(0));", '<p>{{ a }}</p>'), /format!\("\{:\.0\}"/);
probe('B8', 'computed doubled (counter): getter shape', counterCompile,
  component("import {ref, computed} from 'vue'; const count = ref(0); const doubled = computed(() => count.value * 2);", '<p>{{ doubled }}</p>'), /pub fn doubled|cache/i);

say('');
say('== C. Generated application and CLI (README "Try it", "generated/", evidence/verification.txt L2)');
const genCode = counterCompile(readFileSync(join(FIX, 'examples/Counter.vue'), 'utf8'));
const genApp = readFileSync(join(FIX, 'generated/src/app.rs'), 'utf8');
say(`compile(examples/Counter.vue) === generated/src/app.rs: ${genCode === genApp}`);
const work = mkdtempSync(join(tmpdir(), 'vn-g0a-probe-'));
try {
  const cli = join(FIX, 'compiler/cli.mjs');
  const vue = join(FIX, 'examples/Counter.vue');
  const dest = join(work, 'build-counter-2');
  const first = spawnSync('node', [cli, vue, dest], {encoding: 'utf8'});
  say(`cli first run exit=${first.status} stdout="${first.stdout.trim()}"`);
  for (const rel of ['src/app.rs', 'src/lib.rs', 'src/main.rs', 'Cargo.toml', 'index.html']) {
    const same = existsSync(join(dest, rel)) && sha256(readFileSync(join(dest, rel))) === sha256(readFileSync(join(FIX, 'generated', rel)));
    say(`  cli output ${rel} identical to generated/${rel}: ${same}`);
  }
  const second = spawnSync('node', [cli, vue, dest], {encoding: 'utf8'});
  say(`cli second run into the same directory: exit=${second.status} stderr="${second.stderr.trim()}"`);
  const mainRs = readFileSync(join(dest, 'src/main.rs'), 'utf8');
  say(`cli main.rs starts eframe host: ${/eframe::run_native/.test(mainRs)}`);
} finally {
  rmSync(work, {recursive: true, force: true});
}
const lock = readFileSync(join(FIX, 'generated/Cargo.lock'), 'utf8');
say(`generated/Cargo.lock eframe version: ${/name = "eframe"\nversion = "([^"]+)"/.exec(lock)?.[1]}; wasm-bindgen: ${/name = "wasm-bindgen"\nversion = "([^"]+)"/.exec(lock)?.[1]}`);
say(`generated/Cargo.toml enables accesskit feature: ${/accesskit/.test(readFileSync(join(FIX, 'generated/Cargo.toml'), 'utf8'))}`);
const jsEngine = /^name = "(quickjs|rquickjs|boa_engine|v8|deno_core|mozjs|wry|tao|webkit2gtk)"$/m.exec(lock);
say(`generated/Cargo.lock names a JavaScript engine or webview crate: ${jsEngine ? jsEngine[1] : 'no'}`);
const runtimeTests = readFileSync(join(FIX, 'generated/tests/runtime.rs'), 'utf8');
say(`generated/tests/runtime.rs #[test] count (static, not run): ${(runtimeTests.match(/#\[test\]/g) ?? []).length}`);

say('');
say('== D. Toolchain and dependency versions (README "Tool versions")');
for (const pkg of ['@vue/compiler-sfc', '@babel/parser', '@vue/compiler-dom']) {
  const v = JSON.parse(readFileSync(join(PROJECT, 'node_modules', pkg, 'package.json'), 'utf8')).version;
  say(`${pkg} installed version: ${v}`);
}
const rustc = spawnSync('bash', ['-c', 'source "$0" && rustc --version', join(PROJECT, 'scripts/env.sh')], {cwd: PROJECT, encoding: 'utf8'});
say(`project rustc (scripts/env.sh): ${rustc.stdout.trim()} exit=${rustc.status}`);
say(`node: ${process.version}`);

say('');
say('== E. Quick Quote compiler (zip prototype) for the same probes');
probe('E1', '@[click]="inc" (quick-quote)', quickQuoteCompile,
  component("import {ref} from 'vue'; const count = ref(0); function inc() { count.value++ }", '<button @[click]="inc">x</button>'), /clicked/);
probe('E2', '<button @click="inc"> (quick-quote, static baseline)', quickQuoteCompile,
  component("import {ref} from 'vue'; const count = ref(0); function inc() { count.value++ }", '<button @click="inc">x</button>'), /clicked/);

say('');
say('== F. Platform hosts and recorded test names (README "Platform status", evidence/compiler-tests.txt)');
const hostHits = spawnSync('find', ['.', '(', '-path', './node_modules', '-o', '-path', './toolchain', '-o', '-path', './.npm-cache', '-o', '-path', './target', '-o', '-path', './planning', ')', '-prune', '-o', '(', '-iname', '*ios*', '-o', '-iname', '*android*', '-o', '-iname', '*xcode*', '-o', '-iname', '*gradle*', ')', '-print'], {cwd: PROJECT, encoding: 'utf8'});
say(`iOS/Android/Xcode/Gradle host files in the project (name search, excluding node_modules, toolchain, target, planning): ${hostHits.stdout.trim() || 'none'}`);
const recorded = readFileSync(join(FIX, 'evidence/compiler-tests.txt'), 'utf8').split('\n')
  .filter(l => l.startsWith('\u2714 ')).map(l => l.replace(/^\u2714 /, '').replace(/ \([0-9.]+ms\)$/, ''));
const live = spawnSync('node', ['--test', join(FIX, 'tests/compiler.test.mjs')], {encoding: 'utf8'}).stdout.split('\n')
  .filter(l => l.startsWith('\u2714 ')).map(l => l.replace(/^\u2714 /, '').replace(/ \([0-9.]+ms\)$/, ''));
const same = recorded.length === live.length && recorded.every((n, i) => n === live[i]);
say(`recorded evidence/compiler-tests.txt: ${recorded.length} test names; live run: ${live.length}; identical name list and order: ${same}`);

console.log(out.join('\n'));
