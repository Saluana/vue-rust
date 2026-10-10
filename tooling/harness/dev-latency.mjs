// Edit-to-ready latency of `vue-native dev` (WP 3.7, part B). The loop runs on a copy of an example app. Each of N edits
// rewrites the app's <h1> text in src/App.vue; the loop restarts the app, and the restarted app must show the new text
// and the same state element as before the edit (so the state was carried over). The loop's own events give the times:
//   editToReadyMs  newest modification time of the changed file -> the restarted app's mounted reply (what is measured)
//   checkMs        the check (compiler diagnostics, and the type checker unless --no-types)
//   compileMs      the compile inside the build; cargoMs: the cargo build of the generated crate (dominant)
//   startMs        spawn of the app process -> its mounted reply
// The first build of the session is not among the edits; it is reported on its own. Statistics: min, p50 (nearest rank:
// the ceil(p * N)-th smallest value) and max. Budget (design.md, Build row): native warm incremental code build/restart
// <= 5 s, measured on a 100-SFC fixture; the examples here are 1 and 3 SFCs, so the comparison is indicative only.
//   node tooling/harness/dev-latency.mjs [--app counter|quick-quote] [--edits 10] [--no-types] [--no-snapshot] [--budget-ms 5000] [--json]
// Negative controls: --no-snapshot must fail the state check (the restarted app loses its state); --budget-ms 1 must
// report the verdict "over".
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { toolEnv } from '../cli/build.mjs';
import { startDev } from '../cli/dev.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const EXAMPLES = path.resolve(here, '..', '..', 'examples');

// The example apps: the heading the edits change, the element that holds the state, and the interaction before the
// first edit (so that the state is not the initial one).
const APPS = {
  counter: { heading: 'Counter', state: '#count', prepare: [{ op: 'click', selector: '#increment' }, { op: 'click', selector: '#increment' }] },
  'quick-quote': { heading: 'Quick quote', state: '#total', prepare: [{ op: 'click', selector: '#remove-1' }] },
};

const { values } = parseArgs({
  options: {
    app: { type: 'string', default: 'counter' },
    edits: { type: 'string', default: '10' },
    'no-types': { type: 'boolean', default: false },
    // Negative control of the state check: without the snapshot the restarted app loses its state, and the run must fail.
    'no-snapshot': { type: 'boolean', default: false },
    // The budget the verdict is compared with. A budget that no edit can meet (1 ms) is the negative control of the verdict.
    'budget-ms': { type: 'string', default: '5000' },
    json: { type: 'boolean', default: false },
  },
  strict: true,
});

const app = APPS[values.app];
if (app === undefined) throw new Error(`unknown app "${values.app}" (choose ${Object.keys(APPS).join(' or ')})`);
const edits = Number(values.edits);
if (!Number.isInteger(edits) || edits < 1) throw new Error(`--edits must be a positive integer, not "${values.edits}"`);
const budgetMs = Number(values['budget-ms']);
if (!Number.isInteger(budgetMs) || budgetMs < 1) throw new Error(`--budget-ms must be a positive integer, not "${values['budget-ms']}"`);

// A copy of the example in the target directory (the edits must not touch the repository).
const work = path.join(path.resolve(toolEnv().CARGO_TARGET_DIR), 'dev-latency', values.app);
fs.rmSync(work, { recursive: true, force: true });
fs.cpSync(path.join(EXAMPLES, values.app), work, { recursive: true });
const appFile = path.join(work, 'src', 'App.vue');

// Rewrites the single <h1> of App.vue. The check that there is exactly one keeps the edit from being vacuous.
function setHeading(text) {
  const src = fs.readFileSync(appFile, 'utf8');
  const matches = src.match(/<h1>[^<]*<\/h1>/g) ?? [];
  if (matches.length !== 1) throw new Error(`expected one <h1> in App.vue, found ${matches.length}`);
  fs.writeFileSync(appFile, src.replace(/<h1>[^<]*<\/h1>/, `<h1>${text}</h1>`));
}

const quiet = { out: { write: () => {} }, err: { write: () => {} } };
const session = startDev({ root: work, entry: 'src/App.vue', types: !values['no-types'], snapshot: !values['no-snapshot'], pollMs: 100, io: quiet, name: values.app });
const firstBuild = {};
try {
  await session.started;
  const init = session.events.find((e) => e.event === 'ready');
  const built = session.events.find((e) => e.event === 'built');
  if (init === undefined || built === undefined) throw new Error('the first build did not start the app');
  Object.assign(firstBuild, { startMs: init.startMs, checkMs: built.checkMs, compileMs: built.compileMs, cargoMs: built.cargoMs });
  await session.steps(app.prepare);
  const before = (await session.steps([{ op: 'html', selector: app.state }]))[0].html;

  const records = [];
  for (let i = 1; i <= edits; i++) {
    const text = `${app.heading} edit ${i}`;
    const from = session.events.length;
    const cycle = session.nextIdle();
    setHeading(text);
    const outcome = (await cycle).outcome;
    const since = session.events.slice(from);
    const ready = since.filter((e) => e.event === 'ready').at(-1);
    const built2 = since.filter((e) => e.event === 'built').at(-1);
    if (outcome !== 'ready' || ready === undefined || built2 === undefined) {
      throw new Error(`edit ${i}: the cycle ended with "${outcome}"; events: ${since.map((e) => e.event).join(' ')}`);
    }
    const shown = (await session.steps([{ op: 'snapshot', name: 'verify' }]))[0].html;
    if (!shown.includes(text)) throw new Error(`edit ${i}: the restarted app does not show "${text}"`);
    const state = (await session.steps([{ op: 'html', selector: app.state }]))[0].html;
    if (state !== before) throw new Error(`edit ${i}: the state element changed across the restart`);
    records.push({
      edit: i,
      editToReadyMs: ready.editToReadyMs,
      startMs: ready.startMs,
      checkMs: built2.checkMs,
      compileMs: built2.compileMs,
      cargoMs: built2.cargoMs,
      restored: ready.restored.length,
    });
  }

  const stat = (key) => {
    const v = records.map((r) => r[key]).sort((a, b) => a - b);
    const n = v.length;
    return { min: v[0], p50: v[Math.ceil(0.5 * n) - 1], max: v[n - 1] };
  };
  const summary = {
    editToReadyMs: stat('editToReadyMs'),
    checkMs: stat('checkMs'),
    compileMs: stat('compileMs'),
    cargoMs: stat('cargoMs'),
    startMs: stat('startMs'),
  };
  const report = {
    app: values.app,
    edits,
    types: !values['no-types'],
    snapshot: !values['no-snapshot'],
    node: process.version,
    cpus: os.cpus().length,
    budgetMs,
    firstBuild,
    records,
    summary,
  };
  if (values.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    const row = (label, s) => `  ${label.padEnd(18)} ${String(s.min).padStart(7)} ${String(s.p50).padStart(7)} ${String(s.max).padStart(7)}`;
    process.stdout.write(`dev latency: ${values.app}, ${edits} edits, types ${values['no-types'] ? 'skipped' : 'checked'}, node ${process.version}\n`);
    process.stdout.write(`  ${'metric (ms)'.padEnd(18)} ${'min'.padStart(7)} ${'p50'.padStart(7)} ${'max'.padStart(7)}\n`);
    process.stdout.write(`${row('edit to ready', summary.editToReadyMs)}\n`);
    process.stdout.write(`${row('check (types)', summary.checkMs)}\n`);
    process.stdout.write(`${row('compile', summary.compileMs)}\n`);
    process.stdout.write(`${row('cargo', summary.cargoMs)}\n`);
    process.stdout.write(`${row('app start', summary.startMs)}\n`);
    process.stdout.write(`first build (not an edit): check ${firstBuild.checkMs} ms, compile ${firstBuild.compileMs} ms, cargo ${firstBuild.cargoMs} ms, start ${firstBuild.startMs} ms\n`);
    const verdict = summary.editToReadyMs.max <= report.budgetMs ? 'within' : 'over';
    process.stdout.write(`max edit to ready ${summary.editToReadyMs.max} ms is ${verdict} the ${budgetMs} ms budget (indicative: see the header)\n`);
  }
} finally {
  await session.stop();
}
