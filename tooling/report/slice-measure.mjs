#!/usr/bin/env node
// Task 4.5: measured startup, memory and edit latency of the slice apps (examples/counter and examples/quick-quote).
// Builds both apps once as RELEASE headless binaries (tooling/cli/build.mjs), runs them with and without --timing (the flag is
// documented in the header of tooling/cli/headless/main.rs), and prints a table, or with --json one document with the summaries
// and the raw samples in nanoseconds. Everything runs in this container next to other jobs: the output describes the machine.
// Usage: node tooling/report/slice-measure.mjs [--json] [--startup-runs N] [--edit-runs N] [--iterations N]
//   --startup-runs N  rounds per app for the startup and memory figures, each a default run and a --timing run (default 100)
//   --edit-runs N     process runs per app for the edit latency (default 5)
//   --iterations N    workload iterations inside each edit run (default 200)
// Exit status: 0 every check passed; 1 a build, a run or a check failed; 2 a usage error.
// docs/evidence/slice-measurements.md says what each figure is, and what it is not.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { buildHeadlessApp, toolEnv } from '../cli/build.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const TIMING_FLAG = '--timing';
const TIMING_PREFIX = 'vn-timing ';
const RUN_TIMEOUT_MS = 600000;
// The step lines of a long edit run are a few hundred KB, and spawnSync's default buffer is 1 MiB.
const MAX_OUTPUT_BYTES = 256 * 1024 * 1024;
const APPS = ['counter', 'quick-quote'];
const TYPED_NAME = 'Ada Lovelace';
const DEFAULTS = { json: false, startupRuns: 100, editRuns: 5, iterations: 200 };
const OPTION_KEYS = new Map([['--startup-runs', 'startupRuns'], ['--edit-runs', 'editRuns'], ['--iterations', 'iterations']]);
const USAGE = 'usage: node tooling/report/slice-measure.mjs [--json] [--startup-runs N] [--edit-runs N] [--iterations N]';

class UsageError extends Error {}

// ---- workloads -------------------------------------------------------------------------------------------------------

// One Quick Quote iteration: a typing sequence (one input event per character), the demo's refused and accepted quantity
// and price edits, and hiding and showing the form. It ends in the state it started in, so iterations repeat: the customer
// is empty, line 2 has quantity 1, line 1 costs 12.50 and the form is shown.
export function quickQuoteIteration() {
  const typed = Array.from({ length: TYPED_NAME.length }, (_, i) => TYPED_NAME.slice(0, i + 1));
  return [
    ...typed.map((value) => ({ op: 'input', selector: '#customer', value })),
    { op: 'input', selector: '#customer', value: '' },
    { op: 'input', selector: '#quantity-2', value: 'abc' }, // refused: the error line appears
    { op: 'input', selector: '#quantity-2', value: '3' }, // accepted: the total becomes $39.97
    { op: 'input', selector: '#quantity-2', value: '1' }, // accepted: the total is $29.99 again
    { op: 'input', selector: '#price-1', value: '-1' }, // refused
    { op: 'input', selector: '#price-1', value: '12.50' }, // accepted
    { op: 'click', selector: '#toggle' }, // hides the form: the editor is unmounted
    { op: 'click', selector: '#toggle' }, // shows it again: the editor is mounted anew
  ];
}

// One Counter iteration: six clicks (the threshold message shows above five) and a reset.
export function counterIteration() {
  return [...Array.from({ length: 6 }, () => ({ op: 'click', selector: '#increment' })), { op: 'click', selector: '#reset' }];
}

const ITERATION = { counter: counterIteration, 'quick-quote': quickQuoteIteration };

// The script of an edit run: a snapshot, the iterations, and a final snapshot. Only click and input steps are timed.
export function editScript(app, iterations) {
  const steps = [{ op: 'snapshot', name: 'initial' }];
  for (let i = 0; i < iterations; i++) steps.push(...ITERATION[app]());
  steps.push({ op: 'snapshot', name: 'final' });
  return steps;
}

// ---- checks of the rendered state (hand arithmetic from the demo specifications) --------------------------------------

// Vue's scope stamps (data-v-<hash>="" and data-v-app="") are removed before the reading, as tests/cli/headless.test.mjs does.
const withoutStamps = (html) => html.replace(/ data-v-(?:[0-9a-f]{8}|app)=""/g, '');

// The text of element #id in a snapshot (the reading tests/cli/headless.test.mjs uses).
function textOf(html, id) {
  const m = new RegExp(`id="${id}"[^>]*>([^<]*)<`).exec(html);
  return m ? m[1] : null;
}

function expectEqual(problems, what, expected, actual) {
  if (actual !== expected) problems.push(`${what}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
}

// The Quick Quote state at the start and at the end of every iteration: two lines (2 x 1250 + 1 x 499 = 2999 cents), the
// form shown and no error line. Returns the mismatches; an empty list means the state is right.
export function checkQuickQuoteHtml(rawHtml) {
  const html = withoutStamps(rawHtml);
  const problems = [];
  expectEqual(problems, '#total', 'Total: $29.99', textOf(html, 'total'));
  expectEqual(problems, 'line ids', 'line-1,line-2', [...html.matchAll(/id="(line-\d+)"/g)].map((m) => m[1]).join(','));
  expectEqual(problems, 'form shown', true, /id="editor"/.test(html));
  expectEqual(problems, 'error lines', 0, (html.match(/class="error"/g) ?? []).length);
  return problems;
}

// The Counter state at the start and at the end of every iteration: count 0, double 0, reset disabled, no threshold message.
export function checkCounterHtml(rawHtml) {
  const html = withoutStamps(rawHtml);
  const problems = [];
  expectEqual(problems, '#count', 'Count: 0', textOf(html, 'count'));
  expectEqual(problems, '#doubled', 'Double: 0', textOf(html, 'doubled'));
  expectEqual(problems, '#reset disabled', true, /<button id="reset" disabled="">/.test(html));
  expectEqual(problems, 'threshold message', false, /id="many"/.test(html));
  return problems;
}

const CHECK = { counter: checkCounterHtml, 'quick-quote': checkQuickQuoteHtml };
// A deliberate wrong state for the negative control: the check must report exactly one mismatch for it.
const WRONG_STATE = { counter: ['Count: 0', 'Count: 1'], 'quick-quote': ['Total: $29.99', 'Total: $39.97'] };

// ---- --timing output -------------------------------------------------------------------------------------------------

const ALLOWED_OPS = ['click', 'input'];
// The numbers that each phase of the --timing output carries (tooling/cli/headless/main.rs). vm_rss_kib and vm_hwm_kib
// are numbers on Linux and null elsewhere, so they are checked separately.
const PHASE_NUMBERS = {
  mounted: ['main_to_mounted_ns', 'live_objects', 'bytes_estimate', 'allocated_total'],
  step: ['step', 'ns', 'collect_ns'],
  exit: ['steps', 'main_to_end_ns', 'live_objects', 'bytes_estimate', 'allocated_total'],
};

const isNumber = (v) => typeof v === 'number' && Number.isFinite(v);

// One stderr line of the runner. Returns the parsed object for a "vn-timing " line and null for any other line. A timing
// line that is malformed, or lacks a field of its phase, throws: a silent null would hide a broken measurement.
export function parseTimingLine(line) {
  if (!line.startsWith(TIMING_PREFIX)) return null;
  let timing;
  try {
    timing = JSON.parse(line.slice(TIMING_PREFIX.length));
  } catch (err) {
    throw new Error(`malformed vn-timing line (${err.message}): ${line}`);
  }
  const phase = timing?.phase;
  if (typeof phase !== 'string' || !Object.hasOwn(PHASE_NUMBERS, phase)) {
    throw new Error(`vn-timing line with an unknown phase: ${line}`);
  }
  for (const key of PHASE_NUMBERS[phase]) {
    if (!isNumber(timing[key])) throw new Error(`vn-timing ${phase} line without a number "${key}": ${line}`);
  }
  if (phase === 'step') {
    if (!ALLOWED_OPS.includes(timing.op)) throw new Error(`vn-timing step line with op ${JSON.stringify(timing.op)}: ${line}`);
    if (typeof timing.selector !== 'string') throw new Error(`vn-timing step line without a selector: ${line}`);
  } else {
    for (const key of ['vm_rss_kib', 'vm_hwm_kib']) {
      if (timing[key] !== null && !isNumber(timing[key])) throw new Error(`vn-timing ${phase} line with a bad "${key}": ${line}`);
    }
  }
  return timing;
}

// All timing lines of a run's stderr, in order.
export function parseTimingOutput(stderr) {
  return stderr.split('\n').map((line) => parseTimingLine(line)).filter((t) => t !== null);
}

// ---- statistics -------------------------------------------------------------------------------------------------------

// Nearest-rank percentile of ascending samples: the sample at rank ceil(p * n / 100), counting from 1. The result is always
// a measured sample, with no interpolation. p * n is computed before the division, because (p / 100) * n can land just above
// an integer (0.28 * 25 is 7.000000000000001), which would move an exact rank up by one.
export function percentile(sorted, p) {
  if (sorted.length === 0) throw new RangeError('percentile of no samples');
  if (!(p > 0 && p <= 100)) throw new RangeError(`percentile ${p} is outside (0, 100]`);
  return sorted[Math.ceil((p * sorted.length) / 100) - 1];
}

// n, min, p50, p90, p99 and max of the samples (nearest rank).
export function summarize(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return {
    n: sorted.length,
    min: sorted[0],
    p50: percentile(sorted, 50),
    p90: percentile(sorted, 90),
    p99: percentile(sorted, 99),
    max: sorted[sorted.length - 1],
  };
}

// As summarize, but null when a sample is missing (vm_* values are null off Linux).
function summaryOrNull(values) {
  return values.every(isNumber) ? summarize(values) : null;
}

// ---- running the binaries ---------------------------------------------------------------------------------------------

// Runs a headless binary on a script file, with or without --timing. wallNs is the Node-side wall time of the whole process
// (spawn to exit), from process.hrtime.bigint(). A non-zero exit throws with the process's stderr.
function runBinary(binary, scriptFile, timing) {
  const started = process.hrtime.bigint();
  const r = spawnSync(binary, timing ? [TIMING_FLAG, scriptFile] : [scriptFile], {
    encoding: 'utf8', maxBuffer: MAX_OUTPUT_BYTES, timeout: RUN_TIMEOUT_MS,
  });
  const wallNs = Number(process.hrtime.bigint() - started);
  if (r.error) throw new Error(`cannot run ${binary}: ${r.error.message}`);
  if (r.status !== 0) {
    throw new Error(`${path.basename(binary)}${timing ? ' --timing' : ''} exited with ${r.status ?? r.signal}: ${r.stderr.trim()}`);
  }
  return { stdout: r.stdout, timing: timing ? parseTimingOutput(r.stderr) : [], wallNs };
}

// The one line of a phase (the runner prints exactly one mounted line and one exit line).
function onePhase(timing, phase) {
  const found = timing.filter((t) => t.phase === phase);
  if (found.length !== 1) throw new Error(`expected one "${phase}" line, found ${found.length}`);
  return found[0];
}

// Startup: `runs` process runs of an empty script, that is a process which mounts the app and exits. Each round runs the
// binary twice, once as a default run (no --timing: its Node wall time from spawn to exit is the plain figure) and once with
// --timing (the in-process time from the first statement of main() to mounted, the memory and the heap). The two alternate,
// so that both see the same machine state.
function measureStartup(binary, scriptFile, runs) {
  const out = { plainWallNs: [], mountedNs: [], timedWallNs: [], vmHwmKib: [], vmRssKib: [], liveObjects: [], bytesEstimate: [] };
  for (let i = 0; i < runs; i++) {
    const plain = runBinary(binary, scriptFile, false);
    if (plain.stdout.trim() !== '[]') throw new Error(`startup run ${i}: expected an empty result, got ${plain.stdout.trim()}`);
    out.plainWallNs.push(plain.wallNs);
    const r = runBinary(binary, scriptFile, true);
    if (r.stdout.trim() !== '[]') throw new Error(`startup run ${i} with --timing: expected an empty result, got ${r.stdout.trim()}`);
    const mounted = onePhase(r.timing, 'mounted');
    const exit = onePhase(r.timing, 'exit');
    out.mountedNs.push(mounted.main_to_mounted_ns);
    out.timedWallNs.push(r.wallNs);
    out.vmHwmKib.push(exit.vm_hwm_kib);
    out.vmRssKib.push(exit.vm_rss_kib);
    out.liveObjects.push(mounted.live_objects);
    out.bytesEstimate.push(mounted.bytes_estimate);
  }
  return out;
}

// Edit latency: `runs` process runs of the workload, each with --timing. Every run must print the same stdout as one plain
// run of the same script (the flag must not change the result), must time every click and input, and must end, like it
// started, in the hand-computed state. failures lists what did not hold, by kind: result, events or state.
function measureEdit(app, binary, scriptFile, iterations, runs) {
  const plain = runBinary(binary, scriptFile, false);
  const expectedEvents = iterations * ITERATION[app]().length;
  const events = [];
  const perRun = { vmHwmKib: [], vmRssKib: [], liveObjects: [], bytesEstimate: [], wallNs: [] };
  const failures = [];
  let finalHtml = null;
  for (let run = 0; run < runs; run++) {
    const r = runBinary(binary, scriptFile, true);
    if (r.stdout !== plain.stdout) failures.push({ kind: 'result', message: `run ${run} printed a different result` });
    const steps = r.timing.filter((t) => t.phase === 'step');
    if (steps.length !== expectedEvents) failures.push({ kind: 'events', message: `run ${run} timed ${steps.length} events, expected ${expectedEvents}` });
    const results = JSON.parse(r.stdout);
    for (const name of ['initial', 'final']) {
      const snapshot = results.find((x) => x.name === name);
      if (!snapshot) throw new Error(`run ${run}: the result has no "${name}" snapshot`);
      for (const p of CHECK[app](snapshot.html)) failures.push({ kind: 'state', message: `run ${run} ${name}: ${p}` });
      if (name === 'final') finalHtml = snapshot.html;
    }
    for (const s of steps) events.push({ op: s.op, selector: s.selector, ns: s.ns, collectNs: s.collect_ns });
    const exit = onePhase(r.timing, 'exit');
    perRun.vmHwmKib.push(exit.vm_hwm_kib);
    perRun.vmRssKib.push(exit.vm_rss_kib);
    perRun.liveObjects.push(exit.live_objects);
    perRun.bytesEstimate.push(exit.bytes_estimate);
    perRun.wallNs.push(r.wallNs);
  }
  return { events, perRun, failures, expectedEvents, runs, finalHtml };
}

// ---- report ----------------------------------------------------------------------------------------------------------

function machineInfo() {
  const cpus = os.cpus();
  const rustc = spawnSync('rustc', ['--version'], { encoding: 'utf8', env: toolEnv() });
  return {
    cpu: cpus[0]?.model ?? 'unknown',
    vcpus: cpus.length,
    memoryBytes: os.totalmem(),
    kernel: `${os.type()} ${os.release()}`,
    node: process.version,
    rustc: rustc.status === 0 ? rustc.stdout.trim() : 'unknown',
    loadavgStart: os.loadavg(),
  };
}

// The rows of the edit-latency table: one per event type (input, click), then one per target of that type.
function editRows(events) {
  const rows = [];
  for (const op of ALLOWED_OPS) {
    const ofType = events.filter((e) => e.op === op);
    if (ofType.length === 0) continue;
    rows.push({ op, selector: null, summary: summarize(ofType.map((e) => e.ns)) });
    for (const selector of [...new Set(ofType.map((e) => e.selector))]) {
      const ofTarget = ofType.filter((e) => e.selector === selector).map((e) => e.ns);
      rows.push({ op, selector, summary: summarize(ofTarget) });
    }
  }
  return rows;
}

function measure(opts, progress) {
  const machine = machineInfo();
  const targetDir = path.resolve(toolEnv().CARGO_TARGET_DIR);
  const scripts = path.join(targetDir, 'slice-measure', 'scripts');
  fs.mkdirSync(scripts, { recursive: true });
  const apps = {};
  const checks = [];
  for (const app of APPS) {
    progress(`building ${app} (release, headless)`);
    const { dir, manifest } = buildHeadlessApp({
      root: path.join(ROOT, 'examples', app), entry: 'src/App.vue', out: path.join(targetDir, 'slice-measure', app), name: app,
    });
    const binary = path.join(dir, manifest.binary);
    const emptyScript = path.join(scripts, `${app}-empty.json`);
    fs.writeFileSync(emptyScript, '{"steps":[]}\n');
    const editScriptFile = path.join(scripts, `${app}-edit.json`);
    fs.writeFileSync(editScriptFile, `${JSON.stringify({ steps: editScript(app, opts.iterations) })}\n`);

    progress(`startup: ${opts.startupRuns} process runs of ${app}`);
    const startup = measureStartup(binary, emptyScript, opts.startupRuns);
    progress(`edit latency: ${opts.editRuns} process runs of ${app}, ${opts.iterations} iterations each`);
    const edit = measureEdit(app, binary, editScriptFile, opts.iterations, opts.editRuns);

    // The state check must reject a wrong state, or the empty failure lists would prove nothing: one changed element of the
    // real final snapshot must give exactly one mismatch.
    const [from, to] = WRONG_STATE[app];
    const wrong = CHECK[app](edit.finalHtml.replace(from, to));
    checks.push({ name: `${app}: the state check rejects a wrong state (negative control)`, ok: wrong.length === 1, detail: wrong.join('; ') });
    const failed = (kind) => edit.failures.filter((f) => f.kind === kind).map((f) => f.message).join('; ');
    checks.push({ name: `${app}: every edit run prints the result of a plain run`, ok: failed('result') === '', detail: failed('result') });
    checks.push({ name: `${app}: every edit run times every click and input`, ok: failed('events') === '', detail: failed('events') });
    checks.push({ name: `${app}: initial and final states match the hand-computed state`, ok: failed('state') === '', detail: failed('state') });

    apps[app] = {
      binary: manifest.binary,
      startup: {
        runs: opts.startupRuns,
        mountedNs: { values: startup.mountedNs, summary: summarize(startup.mountedNs) },
        plainWallNs: { values: startup.plainWallNs, summary: summarize(startup.plainWallNs) },
        timedWallNs: { values: startup.timedWallNs, summary: summarize(startup.timedWallNs) },
        vmHwmKib: { values: startup.vmHwmKib, summary: summaryOrNull(startup.vmHwmKib) },
        vmRssKib: { values: startup.vmRssKib, summary: summaryOrNull(startup.vmRssKib) },
        liveObjects: { values: startup.liveObjects, summary: summarize(startup.liveObjects) },
        bytesEstimate: { values: startup.bytesEstimate, summary: summarize(startup.bytesEstimate) },
      },
      edit: {
        runs: edit.runs,
        iterations: opts.iterations,
        eventsPerRun: edit.expectedEvents,
        byEvent: editRows(edit.events).map(({ op, selector, summary }) => ({ op, selector, summary })),
        collectNs: summarize(edit.events.map((e) => e.collectNs)),
        perRun: {
          vmHwmKib: edit.perRun.vmHwmKib,
          vmRssKib: edit.perRun.vmRssKib,
          liveObjects: edit.perRun.liveObjects,
          bytesEstimate: edit.perRun.bytesEstimate,
          wallNs: edit.perRun.wallNs,
        },
        samples: edit.events.map((e) => [e.op, e.selector, e.ns, e.collectNs]),
      },
    };
  }
  return {
    tool: 'tooling/report/slice-measure.mjs',
    settings: { startupRuns: opts.startupRuns, editRuns: opts.editRuns, iterations: opts.iterations },
    machine: { ...machine, loadavgEnd: os.loadavg() },
    apps,
    checks,
  };
}

const us = (ns) => (ns / 1000).toFixed(3);

function table(headers, rows) {
  const all = [headers, ...rows];
  const widths = headers.map((_, c) => Math.max(...all.map((r) => String(r[c]).length)));
  return all
    .map((r) => r.map((cell, c) => (c < 2 ? String(cell).padEnd(widths[c]) : String(cell).padStart(widths[c]))).join('  '))
    .join('\n');
}

function renderHuman(report) {
  const m = report.machine;
  const gib = (m.memoryBytes / 2 ** 30).toFixed(1);
  const load = (l) => l.map((x) => x.toFixed(2)).join(' ');
  const out = [
    'vue-native slice measurements (task 4.5): RELEASE headless binaries, run in this container',
    'Headless: no layout, paint, compositor or input device. Edit latency is a lower bound of the framework work.',
    `machine: ${m.cpu}, ${m.vcpus} vCPU, ${gib} GiB RAM, ${m.kernel}, node ${m.node}, ${m.rustc}`,
    `load average (1, 5, 15 min): start ${load(m.loadavgStart)}; end ${load(m.loadavgEnd)}. The container is shared with other jobs.`,
    '',
    `Startup, ${report.settings.startupRuns} rounds per app, each a default run and a --timing run (microseconds).`,
    'main() to mounted: in-process Instant from the first statement of main, in a --timing run. spawn to exit: Node wall time',
    'of the whole process, in a default run and in a --timing run.',
  ];
  const header = ['app', 'metric', 'n', 'min', 'p50', 'p90', 'p99', 'max'];
  const startupRows = [];
  for (const [app, a] of Object.entries(report.apps)) {
    const s = a.startup.mountedNs.summary;
    startupRows.push([app, 'main() to mounted (--timing)', s.n, us(s.min), us(s.p50), us(s.p90), us(s.p99), us(s.max)]);
    const plain = a.startup.plainWallNs.summary;
    startupRows.push([app, 'spawn to exit (default run)', plain.n, us(plain.min), us(plain.p50), us(plain.p90), us(plain.p99), us(plain.max)]);
    const timed = a.startup.timedWallNs.summary;
    startupRows.push([app, 'spawn to exit (--timing run)', timed.n, us(timed.min), us(timed.p50), us(timed.p90), us(timed.p99), us(timed.max)]);
  }
  out.push(table(header, startupRows), '');

  out.push('Memory. Peak RSS is VmHWM from /proc/self/status at exit (Linux only), in KiB. Heap figures are the vn-value');
  out.push('stats after a full collection: live objects, and bytes_estimate (the runtime\'s per-object estimate, not allocator usage).');
  const memRows = [];
  const kibRow = (app, label, values) => {
    const s = summaryOrNull(values);
    return s === null ? [app, label, values.length, 'n/a', '', '', '', ''] : [app, label, s.n, s.min, s.p50, s.p90, s.p99, s.max];
  };
  const intRow = (app, label, s) => [app, label, s.n, s.min, s.p50, s.p90, s.p99, s.max];
  const memHeader = ['app', 'metric', 'n', 'min', 'p50', 'p90', 'p99', 'max'];
  for (const [app, a] of Object.entries(report.apps)) {
    memRows.push(kibRow(app, 'peak RSS (VmHWM) at exit, startup runs', a.startup.vmHwmKib.values));
    memRows.push(intRow(app, 'live objects after mount', a.startup.liveObjects.summary));
    memRows.push(intRow(app, 'bytes_estimate after mount', a.startup.bytesEstimate.summary));
    memRows.push(kibRow(app, 'peak RSS (VmHWM) at exit, edit runs', a.edit.perRun.vmHwmKib));
    memRows.push(kibRow(app, 'live objects after script, edit runs', a.edit.perRun.liveObjects));
    memRows.push(kibRow(app, 'bytes_estimate after script, edit runs', a.edit.perRun.bytesEstimate));
  }
  out.push(table(memHeader, memRows), '');

  out.push(`Edit latency (microseconds). Dispatch of one scripted click or input to the end of the scheduler flush that`);
  out.push(`patches the DOM (the job queue is empty). ${report.settings.editRuns} process runs per app, ${report.settings.iterations} iterations each,`);
  out.push('samples pooled over the runs. "collect" is the full collection after each event, outside that window, which the web host also runs.');
  const editRows = [];
  for (const [app, a] of Object.entries(report.apps)) {
    for (const row of a.edit.byEvent) {
      const label = row.selector === null ? `${row.op} (all)` : `  ${row.op} ${row.selector}`;
      const s = row.summary;
      editRows.push([app, label, s.n, us(s.min), us(s.p50), us(s.p90), us(s.p99), us(s.max)]);
    }
    const c = a.edit.collectNs;
    editRows.push([app, 'collect after each event', c.n, us(c.min), us(c.p50), us(c.p90), us(c.p99), us(c.max)]);
  }
  out.push(table(header, editRows), '');

  out.push('Checks');
  for (const c of report.checks) out.push(`  [${c.ok ? 'ok' : 'FAILED'}] ${c.name}${c.detail ? `: ${c.detail}` : ''}`);
  return out.join('\n');
}

// ---- command line ----------------------------------------------------------------------------------------------------

function parseArgs(argv) {
  const opts = { ...DEFAULTS };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--json') {
      opts.json = true;
      continue;
    }
    if (!OPTION_KEYS.has(arg)) throw new UsageError(`unknown argument ${arg}`);
    const value = argv[++i];
    if (!/^[1-9][0-9]*$/.test(value ?? '')) throw new UsageError(`${arg} needs a positive whole number`);
    opts[OPTION_KEYS.get(arg)] = Number(value);
  }
  return opts;
}

export function main(argv) {
  const progress = (message) => process.stderr.write(`slice-measure: ${message}\n`);
  let opts;
  try {
    opts = parseArgs(argv);
  } catch (err) {
    if (!(err instanceof UsageError)) throw err;
    process.stderr.write(`slice-measure: ${err.message}\n${USAGE}\n`);
    return 2;
  }
  let report;
  try {
    report = measure(opts, progress);
  } catch (err) {
    process.stderr.write(`slice-measure: ${err.message}\n`);
    return 1;
  }
  process.stdout.write(opts.json ? `${JSON.stringify(report)}\n` : `${renderHuman(report)}\n`);
  return report.checks.every((c) => c.ok) ? 0 : 1;
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  process.exitCode = main(process.argv.slice(2));
}
