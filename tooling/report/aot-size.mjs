#!/usr/bin/env node
// Release artifact size and allocation report (WP 3.1). Builds in release mode, through the project's own paths:
//   - the Counter and Quick Quote examples with the CLI: node tooling/cli/vue-native.mjs build <dir> --target web|headless
//   - the browser host wasm: tooling/web/build-wasm.mjs (the shipped variant, without the selftest export)
// For every artifact it reports raw bytes and gzip (level 9) bytes; native binaries also report the stripped size.
// For the native binaries it approximates the contributors by crate from `nm --size-sort -C` (groupSymbols): an
// approximation, not cargo-bloat. For each example it runs one interaction script with the headless binary's --stats flag
// and reports the vn-value heap counters: objects allocated, live objects, the byte estimate and their peaks. The checks
// on the script results are run too, so a report is never written for an interaction that did not happen.
//   node tooling/report/aot-size.mjs [--out <dir>] [--json]
// The output directory defaults to $CARGO_TARGET_DIR/aot-size (or target/aot-size) and is replaced on each run. --json
// prints the summary as JSON on stdout (it is also written to <out>/summary.json).
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { buildWebDist } from '../web/build-wasm.mjs';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const CLI = path.join(ROOT, 'tooling', 'cli', 'vue-native.mjs');

// The interaction scripts. Counter: the threshold sequence of tests/cli/headless.test.mjs. Quick Quote: the whole sequence
// of the same file. The expectations are the demo specification's values, written by hand (Counter: 5 clicks show 5, the
// sixth shows 6 and doubles to 12; reset clears; Quick Quote: 2 x 1250 + 1 x 499 = 2999 cents, quantity 3 on line 2 gives
// 3997 (the total is $39.97 at step 5), and removing line 1 leaves 1497 cents ($14.97 at step 15)).
export const EXAMPLES = Object.freeze([
  {
    name: 'counter',
    dir: path.join(ROOT, 'examples', 'counter'),
    steps: [
      ...Array.from({ length: 5 }, () => ({ op: 'click', selector: '#increment' })),
      { op: 'snapshot', name: 'five' },
      { op: 'click', selector: '#increment' },
      { op: 'snapshot', name: 'six' },
      { op: 'click', selector: '#reset' },
      { op: 'snapshot', name: 'reset' },
    ],
    check(results) {
      const count = (i) => textOf(results[i].html, 'count');
      const doubled = (i) => textOf(results[i].html, 'doubled');
      const expect = [[count(5), 'Count: 5'], [count(7), 'Count: 6'], [doubled(7), 'Double: 12'], [count(9), 'Count: 0']];
      return expect.filter(([got, want]) => got !== want).map(([got, want]) => `expected ${want}, got ${got}`);
    },
  },
  {
    name: 'quick-quote',
    dir: path.join(ROOT, 'examples', 'quick-quote'),
    steps: [
      { op: 'snapshot', name: 'initial' },
      { op: 'input', selector: '#quantity-2', value: 'abc' },
      { op: 'snapshot', name: 'qty-invalid' },
      { op: 'input', selector: '#quantity-2', value: '3' },
      { op: 'html', selector: '#total' },
      { op: 'html', selector: '#status' },
      { op: 'input', selector: '#price-1', value: '-1' },
      { op: 'snapshot', name: 'price-invalid' },
      { op: 'click', selector: '#add-line' },
      { op: 'click', selector: '#toggle' },
      { op: 'snapshot', name: 'hidden' },
      { op: 'click', selector: '#toggle' },
      { op: 'snapshot', name: 'shown' },
      { op: 'click', selector: '#remove-1' },
      { op: 'html', selector: '#total' },
      { op: 'html', selector: '#status' },
    ],
    check(results) {
      // The text of each element. The whole tag is not compared: Vue's scoped-style stamp (data-v-<hash>) differs per build.
      const expect = [
        [textOf(results[0].html, 'total'), 'Total: $29.99'],
        [textOf(results[4].html, 'total'), 'Total: $39.97'],
        [textOf(results[5].html, 'status'), 'Total changed from $29.99 to $39.97'],
        [textOf(results[14].html, 'total'), 'Total: $14.97'],
        [textOf(results[15].html, 'status'), 'Total changed from $39.97 to $14.97'],
      ];
      return expect.filter(([got, want]) => got !== want).map(([got, want]) => `expected ${want}, got ${got}`);
    },
  },
]);

// The text of element #id in an html result (as tests/cli/headless.test.mjs reads it), or null.
export function textOf(html, id) {
  const m = new RegExp(`id="${id}"[^>]*>([^<]*)<`).exec(html);
  return m ? m[1] : null;
}

export const gzipLength = (bytes) => zlib.gzipSync(bytes, { level: 9 }).length;

// Parses `nm --print-size` lines ("<addr> <size-hex> <type> <name>"). Lines without a size are not symbols with bytes.
export function parseNm(text) {
  const out = [];
  for (const line of text.split('\n')) {
    const m = /^[0-9a-fA-F]+ ([0-9a-fA-F]+) (\S) (.+)$/.exec(line);
    if (m) out.push({ size: parseInt(m[1], 16), type: m[2], name: m[3] });
  }
  return out;
}

// Symbol types whose bytes are in the file: text, read-only and initialised data. bss, common and the rest are not.
const FILE_BACKED = new Set([...'TtDdRrVvWwGg']);

// The crate a demangled Rust symbol belongs to, for the approximation of groupSymbols. "<T as Trait>::f" is attributed to the
// crate of T, or of the trait when T is a reference or a generic parameter. Application crates (vn_app_<name>_<hash>) are
// grouped by name without the per-output hash. A name with no path is "(no crate path)".
export function crateOf(name) {
  let n = name.trim();
  if (n.startsWith('<')) {
    const inner = n.slice(1);
    const as = inner.indexOf(' as ');
    const self = as >= 0 ? inner.slice(0, as) : inner.replace(/>::.*$/, '');
    const fromSelf = /^([A-Za-z_][A-Za-z0-9_]*)::/.exec(self);
    if (fromSelf) return crateOf(`${fromSelf[1]}::x`);
    const trait = as >= 0 ? inner.slice(as + 4).replace(/>::.*$/, '') : '';
    return crateOf(trait);
  }
  const m = /^([A-Za-z_][A-Za-z0-9_]*)::/.exec(n);
  if (!m) return '(no crate path)';
  const crate = m[1];
  return crate.startsWith('vn_app_') ? crate.replace(/_[0-9a-f]{8}$/, '') : crate;
}

// Sums the sizes of the file-backed symbols by crate. Returns { totalBytes, symbols, groups } with groups sorted by bytes.
export function groupSymbols(nmText) {
  const groups = new Map();
  let totalBytes = 0;
  let symbols = 0;
  for (const sym of parseNm(nmText)) {
    if (!FILE_BACKED.has(sym.type) || sym.size === 0) continue;
    const crate = crateOf(sym.name);
    const g = groups.get(crate) ?? { crate, bytes: 0, symbols: 0 };
    g.bytes += sym.size;
    g.symbols += 1;
    groups.set(crate, g);
    totalBytes += sym.size;
    symbols += 1;
  }
  const sorted = [...groups.values()].sort((a, b) => b.bytes - a.bytes || (a.crate < b.crate ? -1 : 1));
  return { totalBytes, symbols, groups: sorted.map((g) => ({ ...g, share: totalBytes === 0 ? 0 : g.bytes / totalBytes })) };
}

function run(command, args, options = {}) {
  const r = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, ...options });
  if (r.error) throw new Error(`cannot run ${command}: ${r.error.message}`);
  return r;
}

function mustRun(command, args, what, options) {
  const r = run(command, args, options);
  if (r.status !== 0) throw new Error(`${what} failed (exit ${r.status}):\n${`${r.stderr ?? ''}${r.stdout ?? ''}`.trim().split('\n').slice(-40).join('\n')}`);
  return r;
}

// Builds an example for one CLI target and returns its output directory.
function buildExample(example, target, outRoot) {
  const out = path.join(outRoot, `${example.name}-${target}`);
  fs.rmSync(out, { recursive: true, force: true });
  mustRun(process.execPath, [CLI, 'build', example.dir, '--target', target, '--out', out, '--name', example.name], `build ${example.name} for ${target}`);
  return out;
}

function measureNative(example, binary, outRoot) {
  const raw = fs.readFileSync(binary);
  const stripped = path.join(outRoot, `${example.name}.stripped`);
  mustRun('strip', ['--strip-all', '-o', stripped, binary], `strip ${example.name}`);
  const strippedBytes = fs.readFileSync(stripped);
  fs.rmSync(stripped, { force: true });
  const nm = mustRun('nm', ['--size-sort', '-C', '--print-size', '--defined-only', binary], `nm ${example.name}`);
  return {
    rawBytes: raw.length,
    gzipBytes: gzipLength(raw),
    strippedBytes: strippedBytes.length,
    strippedGzipBytes: gzipLength(strippedBytes),
    contributors: groupSymbols(nm.stdout),
  };
}

// Runs one example's interaction script with --stats. Returns the results and the two heap snapshots.
function runScript(example, binary, scriptFile, stats) {
  fs.writeFileSync(scriptFile, JSON.stringify({ steps: example.steps }));
  const args = [scriptFile, ...(stats ? ['--stats'] : [])];
  const r = run(binary, args);
  if (r.status !== 0) throw new Error(`${example.name} interaction script failed (exit ${r.status}): ${r.stderr.trim()}`);
  const results = JSON.parse(r.stdout);
  let heap = null;
  if (stats) {
    const last = r.stderr.trim().split('\n').pop();
    heap = JSON.parse(last);
  }
  return { results, heap, stdout: r.stdout };
}

function allocationSummary(example, binary, scriptDir) {
  const scriptFile = path.join(scriptDir, `${example.name}-script.json`);
  const plain = runScript(example, binary, scriptFile, false);
  const measured = runScript(example, binary, scriptFile, true);
  const problems = example.check(measured.results);
  if (measured.stdout !== plain.stdout) problems.push('the results with --stats differ from the results without it');
  if (problems.length > 0) throw new Error(`${example.name}: the interaction did not produce the expected results: ${problems.join('; ')}`);
  const { afterMount, end } = measured.heap;
  return {
    steps: example.steps.length,
    checks: 'passed (results match the demo specification; --stats does not change the results)',
    realm: { allocatedTotal: afterMount.allocated_total, liveObjects: afterMount.live_objects, bytesEstimate: afterMount.bytes_estimate },
    script: {
      objectsAllocated: end.allocated_total - afterMount.allocated_total,
      collections: end.collections - afterMount.collections,
      freedTotal: end.freed_total,
    },
    end: {
      allocatedTotal: end.allocated_total,
      liveObjects: end.live_objects,
      bytesEstimate: end.bytes_estimate,
      peakLiveObjects: end.peak_live_objects,
      peakBytesEstimate: end.peak_bytes_estimate,
    },
  };
}

const kib = (n) => `${(n / 1024).toFixed(1)} KiB`;

function textReport(summary) {
  const lines = [];
  lines.push(`release artifact sizes (${summary.toolchain.rustc}; node ${summary.toolchain.node})`);
  lines.push(`${'artifact'.padEnd(34)} ${'raw bytes'.padStart(10)} ${'gzip bytes'.padStart(11)} ${'stripped'.padStart(10)} ${'stripped gzip'.padStart(14)}`);
  for (const a of summary.artifacts) {
    lines.push(`${a.name.padEnd(34)} ${String(a.rawBytes).padStart(10)} ${String(a.gzipBytes).padStart(11)} ${String(a.strippedBytes ?? '-').padStart(10)} ${String(a.strippedGzipBytes ?? '-').padStart(14)}`);
  }
  for (const [name, c] of Object.entries(summary.contributors)) {
    lines.push('');
    lines.push(`approximate contributors of ${name} (file-backed symbol bytes by crate, from nm; total ${c.totalBytes} bytes in ${c.symbols} symbols)`);
    for (const g of c.groups.slice(0, 12)) lines.push(`  ${g.crate.padEnd(34)} ${String(g.bytes).padStart(10)} ${(g.share * 100).toFixed(1).padStart(5)}%  ${g.symbols} symbols`);
  }
  for (const [name, a] of Object.entries(summary.allocation)) {
    lines.push('');
    lines.push(`heap of ${name}: ${a.checks}`);
    lines.push(`  realm creation and mount: ${a.realm.allocatedTotal} objects allocated, ${a.realm.liveObjects} live, ${a.realm.bytesEstimate} bytes (estimate)`);
    lines.push(`  interaction script (${a.steps} steps): ${a.script.objectsAllocated} objects allocated, ${a.script.collections} collections, ${a.script.freedTotal} freed in total`);
    lines.push(`  at the end: ${a.end.liveObjects} live objects, ${a.end.bytesEstimate} bytes (estimate); peak ${a.end.peakLiveObjects} live objects, ${a.end.peakBytesEstimate} bytes (estimate)`);
  }
  lines.push('');
  lines.push(`engine-backed comparison: ${summary.engineComparison.status}. ${summary.engineComparison.reason}`);
  return lines.join('\n');
}

export function main(argv) {
  const opt = (name) => {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const targetRoot = process.env.CARGO_TARGET_DIR ?? path.join(ROOT, 'target');
  const outRoot = path.resolve(opt('--out') ?? path.join(targetRoot, 'aot-size'));
  fs.rmSync(outRoot, { recursive: true, force: true });
  fs.mkdirSync(outRoot, { recursive: true });
  const scratch = path.join(outRoot, 'scratch');
  fs.mkdirSync(scratch, { recursive: true });

  const artifacts = [];
  const contributors = {};
  const allocation = {};

  // 1. The browser host wasm (tooling/web/build-wasm.mjs, shipped variant).
  const [host] = buildWebDist({ outDir: path.join(outRoot, 'web-host'), only: 'shipped' });
  artifacts.push({ name: 'web host wasm (vn_renderer_dom, shipped)', rawBytes: host.manifest.bytes, gzipBytes: host.gzipBytes });

  for (const example of EXAMPLES) {
    // 2. The example as a web app (wasm32, release, the CLI's size profile) and 3. as a headless native binary.
    const web = buildExample(example, 'web', outRoot);
    const webManifest = JSON.parse(fs.readFileSync(path.join(web, 'manifest.json'), 'utf8'));
    artifacts.push({ name: `${example.name} web app wasm`, rawBytes: webManifest.bytes, gzipBytes: webManifest.gzipBytes });

    const headless = buildExample(example, 'headless', outRoot);
    const binary = path.join(headless, example.name);
    const native = measureNative(example, binary, scratch);
    artifacts.push({
      name: `${example.name} headless native (release)`,
      rawBytes: native.rawBytes,
      gzipBytes: native.gzipBytes,
      strippedBytes: native.strippedBytes,
      strippedGzipBytes: native.strippedGzipBytes,
    });
    contributors[`${example.name} headless native`] = native.contributors;

    allocation[example.name] = allocationSummary(example, binary, scratch);
  }

  const summary = {
    schemaVersion: 1,
    toolchain: {
      node: process.version,
      rustc: mustRun('rustc', ['--version'], 'rustc --version').stdout.trim(),
    },
    profiles: {
      web: 'opt-level "z", lto true, codegen-units 1, panic abort (tooling/cli/build.mjs cargoToml)',
      headless: 'lto true, codegen-units 1 (tooling/cli/build.mjs cargoTomlHeadless)',
      webHost: 'workspace release profile (Cargo.toml: debug 0, codegen-units 16)',
    },
    artifacts,
    contributors,
    allocation,
    approximations: [
      'contributors: sum of st_size of file-backed symbols (nm types T t D d R r V v W w G g) grouped by the crate path of the demangled name; not cargo-bloat; generic instances are attributed to the crate named in their path.',
      'raw and gzip bytes are the artifact files; stripped bytes are `strip --strip-all` of a copy of the binary.',
      'heap counters are vn-value estimates (object base, property and element bytes), not allocator bytes. The headless runner collects at each step with --stats, as the web host does at each event.',
    ],
    engineComparison: {
      status: 'not done',
      reason: 'No published QuickJS or Boa size is cited in the repository docs, and no engine-backed build was made (WP 3.1 Part C). See docs/evidence/aot-size-report.md.',
    },
  };
  fs.writeFileSync(path.join(outRoot, 'summary.json'), `${JSON.stringify(summary, null, 2)}\n`);
  fs.rmSync(scratch, { recursive: true, force: true });
  if (argv.includes('--json')) process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
  else process.stdout.write(`${textReport(summary)}\n`);
  return summary;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (err) {
    process.stderr.write(`aot-size: ${err.message}\n`);
    process.exitCode = 1;
  }
}
