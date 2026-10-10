// The vue-native dev loop (WP 3.7, part B): an edit restarts the headless app, and the state of its root <script setup>
// is carried over by binding name. Expected values are hand-derived from the fixture texts (tests/cli/fixtures/dev-state,
// examples/counter) and from the snapshot rules, not taken from the loop's output. Each test edits its own copy of a
// project. Negative controls: the same edit with the snapshot disabled, a same-type edit (the binding is restored), a
// type error (the running app is kept), and a dev form that renders differently (the comparison detects it).
import { describe, test, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { buildHeadlessApp } from '../../tooling/cli/build.mjs';
import { changedFiles, dev, scanSources, startDev } from '../../tooling/cli/dev.mjs';
import { run } from '../../tooling/cli/cli.mjs';
import { EXAMPLES, FIXTURES, ROOT, freshPath, scratch } from './helpers.mjs';

const sessions = [];
after(async () => {
  for (const s of sessions) await s.stop();
});

// A fresh copy of a project (fixture or example) in a scratch directory; returns its root.
function copyProject(from, label) {
  const dir = scratch(`dev-${label}`);
  fs.cpSync(from, dir, { recursive: true });
  return dir;
}

// Replaces one exact text of a project file. The old text must be present exactly once, so that no edit is vacuous.
function editFile(root, rel, from, to) {
  const file = path.join(root, rel);
  const text = fs.readFileSync(file, 'utf8');
  const n = text.split(from).length - 1;
  assert.equal(n, 1, `the edit must match exactly once in ${rel} (it matched ${n} times)`);
  fs.writeFileSync(file, text.replace(from, to));
}

// A session with captured text output. Returns { session, out, err }.
function start(root, options = {}) {
  const out = [];
  const err = [];
  const io = { out: { write: (s) => out.push(s) }, err: { write: (s) => err.push(s) } };
  const session = startDev({ root, entry: 'src/App.vue', types: false, pollMs: 100, io, ...options });
  sessions.push(session);
  return { session, out, err };
}

const lastEvent = (session, name) => session.events.filter((e) => e.event === name).at(-1);

const clicks = (selector, n) => Array.from({ length: n }, () => ({ op: 'click', selector }));

// The html of a step's reply (an html step, or a snapshot step's container html).
const htmlOf = (results, i = 0) => results[i].html;

describe('vue-native dev: a template edit restarts the app and keeps the counter', () => {
  test('the Counter example keeps its count across a template edit (3 clicks, then the count is 3)', async () => {
    const root = copyProject(path.join(EXAMPLES, 'counter'), 'counter');
    const { session } = start(root);
    await session.started;
    const before = await session.steps([...clicks('#increment', 3), { op: 'html', selector: '#count' }]);
    assert.match(htmlOf(before, 3), />Count: 3<\/p>$/);
    const cycle = session.nextIdle();
    editFile(root, 'src/App.vue', '<h1>Counter</h1>', '<h1>Counter, edited</h1>');
    assert.equal((await cycle).outcome, 'ready');
    const after = await session.steps([{ op: 'snapshot', name: 'after' }, { op: 'html', selector: '#count' }]);
    assert.match(htmlOf(after, 0), /Counter, edited/, 'the edit is in the running app');
    assert.match(htmlOf(after, 1), />Count: 3<\/p>$/, 'the count of the previous app is restored');
    assert.deepEqual(lastEvent(session, 'ready').restored, ['count']);
    assert.deepEqual(lastEvent(session, 'ready').skipped, []);
  });

  test('negative control: with the snapshot disabled the same edit resets the count to 0', async () => {
    const root = copyProject(path.join(EXAMPLES, 'counter'), 'counter-nosnap');
    const { session, out } = start(root, { snapshot: false });
    await session.started;
    await session.steps(clicks('#increment', 3));
    const cycle = session.nextIdle();
    editFile(root, 'src/App.vue', '<h1>Counter</h1>', '<h1>Counter, edited</h1>');
    assert.equal((await cycle).outcome, 'ready');
    const after = await session.steps([{ op: 'html', selector: '#count' }]);
    assert.match(htmlOf(after), />Count: 0<\/p>$/);
    assert.deepEqual(lastEvent(session, 'ready').restored, []);
    assert.equal(lastEvent(session, 'snapshot').enabled, false);
    assert.match(out.join(''), /state: not kept \(--no-snapshot\)/);
  });
});

describe('vue-native dev: a closure held in a ref is not serialized', () => {
  test('onTick (a closure in a ref) is left out of the snapshot and named in a warning', async () => {
    const root = copyProject(path.join(FIXTURES, 'dev-state'), 'closure');
    const { session, err } = start(root);
    await session.started;
    await session.steps(clicks('#bump', 2));
    const cycle = session.nextIdle();
    editFile(root, 'src/App.vue', '<h1>State</h1>', '<h1>State two</h1>');
    assert.equal((await cycle).outcome, 'ready');
    const snap = lastEvent(session, 'snapshot');
    assert.deepEqual(snap.dropped, ['onTick: function (closure, not serialized)']);
    assert.deepEqual(Object.keys(snap.state).sort(), ['count', 'label', 'settings']);
    assert.match(err.join(''), /warning: not saved: onTick: function \(closure, not serialized\)\n/);
    // The other bindings are still carried over.
    const after = await session.steps([{ op: 'html', selector: '#count' }, { op: 'html', selector: '#tick' }]);
    assert.match(htmlOf(after, 0), />Count: 2<\/p>$/);
    assert.match(htmlOf(after, 1), />Tick: 102<\/p>$/, 'the new app computes the closure from the restored count');
  });
});

describe('vue-native dev: a binding whose type changed is skipped', () => {
  test('label (string) becomes a number: it is skipped with a warning, count and settings are restored', async () => {
    const root = copyProject(path.join(FIXTURES, 'dev-state'), 'retype');
    const { session, err } = start(root);
    await session.started;
    await session.steps(clicks('#bump', 3));
    const cycle = session.nextIdle();
    editFile(root, 'src/App.vue', "const label = ref('a')", 'const label = ref(7)');
    assert.equal((await cycle).outcome, 'ready');
    const ready = lastEvent(session, 'ready');
    assert.deepEqual(ready.skipped, [{ name: 'label', reason: 'type changed from string to number' }]);
    assert.deepEqual(ready.restored, ['count', 'settings']);
    assert.match(err.join(''), /warning: not restored: label: type changed from string to number\n/);
    const after = await session.steps([{ op: 'html', selector: '#count' }, { op: 'html', selector: '#label' }]);
    assert.match(htmlOf(after, 0), />Count: 3<\/p>$/);
    assert.match(htmlOf(after, 1), />Label: 7<\/p>$/, 'the new initial value of the changed binding');
  });

  test('negative control: a same-type edit (string to string) restores the label instead', async () => {
    const root = copyProject(path.join(FIXTURES, 'dev-state'), 'same-type');
    const { session, err } = start(root);
    await session.started;
    await session.steps(clicks('#bump', 1));
    const cycle = session.nextIdle();
    editFile(root, 'src/App.vue', "const label = ref('a')", "const label = ref('b')");
    assert.equal((await cycle).outcome, 'ready');
    const ready = lastEvent(session, 'ready');
    assert.deepEqual(ready.skipped, []);
    assert.deepEqual(ready.restored, ['count', 'label', 'settings']);
    const after = await session.steps([{ op: 'html', selector: '#label' }]);
    assert.match(htmlOf(after), />Label: a<\/p>$/, 'the old value, not the new initial one');
    assert.doesNotMatch(err.join(''), /not restored/);
  });
});

describe('vue-native dev: the check and the build run on each change', () => {
  test('a type error keeps the running app; the fixed file restarts it with the state', async () => {
    const root = copyProject(path.join(FIXTURES, 'dev-state'), 'type-error');
    const { session } = start(root, { types: true });
    await session.started;
    await session.steps(clicks('#bump', 2));
    let cycle = session.nextIdle();
    editFile(root, 'src/App.vue', "const label = ref('a')", "const label: number = ref('a')");
    assert.equal((await cycle).outcome, 'kept');
    const failed = lastEvent(session, 'check');
    assert.equal(failed.ok, false);
    assert.ok(failed.errors >= 1);
    assert.equal(failed.typesChecked, true);
    const still = await session.steps([{ op: 'html', selector: '#count' }]);
    assert.match(htmlOf(still), />Count: 2<\/p>$/, 'the running app is unchanged');

    cycle = session.nextIdle();
    editFile(root, 'src/App.vue', "const label: number = ref('a')", "const label = ref('a')");
    assert.equal((await cycle).outcome, 'ready');
    assert.equal(lastEvent(session, 'check').ok, true);
    const fixed = await session.steps([{ op: 'html', selector: '#count' }]);
    assert.match(htmlOf(fixed), />Count: 2<\/p>$/);
  });

  test('an app process that ends on its own is reported; the next change starts it without a state', async () => {
    const root = copyProject(path.join(FIXTURES, 'dev-state'), 'killed');
    const { session, err } = start(root);
    await session.started;
    await session.steps(clicks('#bump', 2));
    const exited = session.waitFor((e) => e.event === 'exited');
    session.child.kill('SIGKILL');
    const ev = await exited;
    assert.equal(ev.signal, 'SIGKILL');
    assert.match(err.join(''), /warning: the app process ended \(signal SIGKILL\)/);
    const cycle = session.nextIdle();
    editFile(root, 'src/App.vue', '<h1>State</h1>', '<h1>State two</h1>');
    assert.equal((await cycle).outcome, 'ready');
    assert.deepEqual(lastEvent(session, 'ready').restored, []);
    const after = await session.steps([{ op: 'html', selector: '#count' }]);
    assert.match(htmlOf(after), />Count: 0<\/p>$/);
  });
});

describe('vue-native dev: the dev form of <script setup> renders the same DOM as the inline form', () => {
  const script = { steps: [...clicks('#increment', 2), { op: 'snapshot', name: 'two' }, { op: 'html', selector: '#doubled' }] };
  const buildWith = (devState, label) => {
    const out = freshPath(`dev-form-${label}`);
    const built = buildHeadlessApp({ root: path.join(EXAMPLES, 'counter'), entry: 'src/App.vue', out, name: `form-${label}`, profile: 'dev', devState });
    return path.join(built.dir, `form-${label}`);
  };
  const runOneShot = (binary, spec) => {
    const dir = scratch(`dev-form-script-${path.basename(binary)}`);
    const file = path.join(dir, 'script.json');
    fs.writeFileSync(file, JSON.stringify(spec));
    const r = spawnSync(binary, [file], { encoding: 'utf8', timeout: 60000 });
    assert.equal(r.status, 0, r.stderr);
    return JSON.parse(r.stdout);
  };
  let inline;
  let devForm;
  test('the inline form and the dev form give the same results for the same interaction', () => {
    inline = buildWith(false, 'inline');
    devForm = buildWith(true, 'devform');
    const a = runOneShot(inline, script);
    const b = runOneShot(devForm, script);
    assert.deepEqual(b, a);
    assert.match(a[3].html, /Double: 4<\/p>/, 'two clicks give 2, doubled is 4 (hand-derived)');
  });

  test('negative control: a different number of clicks gives different results', () => {
    const three = { steps: [...clicks('#increment', 3), { op: 'snapshot', name: 'three' }, { op: 'html', selector: '#doubled' }] };
    const a = runOneShot(inline, script);
    const b = runOneShot(devForm, three);
    assert.notDeepEqual(b, a);
  });
});

describe('vue-native dev: helpers and the command line', () => {
  test('scanSources lists the watched files and skips node_modules, dot directories and other extensions', () => {
    const root = copyProject(path.join(FIXTURES, 'dev-state'), 'scan');
    fs.mkdirSync(path.join(root, 'node_modules', 'pkg'), { recursive: true });
    fs.writeFileSync(path.join(root, 'node_modules', 'pkg', 'index.js'), 'export {};\n');
    fs.mkdirSync(path.join(root, '.hidden'));
    fs.writeFileSync(path.join(root, '.hidden', 'a.js'), 'export {};\n');
    fs.writeFileSync(path.join(root, 'notes.txt'), 'not a source\n');
    fs.writeFileSync(path.join(root, 'src', 'util.js'), 'export const n = 1;\n');
    assert.deepEqual([...scanSources(root).keys()].sort(), ['src/App.vue', 'src/util.js']);
    assert.deepEqual([...scanSources(root, [path.join(root, 'src')]).keys()], []);
  });

  test('changedFiles reports changed, added and removed files (negative control: an unchanged scan reports nothing)', () => {
    const before = new Map([['a.vue', { mtimeMs: 1, size: 3 }], ['b.js', { mtimeMs: 2, size: 4 }]]);
    assert.deepEqual(changedFiles(before, new Map(before)), []);
    const after = new Map([['a.vue', { mtimeMs: 5, size: 3 }], ['c.js', { mtimeMs: 6, size: 1 }]]);
    assert.deepEqual(changedFiles(before, after), ['a.vue', 'b.js', 'c.js']);
  });

  test('the command line rejects a missing project and a bad poll interval (status 2, nothing started)', async () => {
    const out = [];
    const err = [];
    const io = { out: { write: (s) => out.push(s) }, err: { write: (s) => err.push(s) } };
    assert.equal(await run(['dev'], io), 2);
    assert.equal(await dev([path.join(FIXTURES, 'dev-state')], { 'poll-ms': '5' }, io), 2);
    assert.match(err.join(''), /--poll-ms must be an integer of at least 10/);
    const help = [];
    assert.equal(run(['--help'], { out: { write: (s) => help.push(s) }, err: { write: () => {} } }), 0);
    assert.match(help.join(''), /dev <entry\.vue\|dir>/);
  });

  test('vue-native dev runs until interrupted: the edit is picked up, SIGINT stops it with status 0', async () => {
    const root = copyProject(path.join(FIXTURES, 'dev-state'), 'cli');
    const child = spawn(process.execPath, [path.join(ROOT, 'tooling', 'cli', 'vue-native.mjs'), 'dev', root, '--no-types', '--poll-ms', '100'], { stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (c) => { stdout += c; });
    child.stderr.on('data', (c) => { stderr += c; });
    const waitFor = (pattern, timeoutMs = 300000) => new Promise((resolve, reject) => {
      const t0 = Date.now();
      const tick = setInterval(() => {
        if (pattern.test(stdout)) { clearInterval(tick); resolve(); }
        else if (Date.now() - t0 > timeoutMs) { clearInterval(tick); reject(new Error(`no ${pattern} in ${timeoutMs} ms; stdout: ${stdout}; stderr: ${stderr}`)); }
      }, 50);
    });
    await waitFor(/ready: app mounted in \d+ ms\n/);
    const edited = waitFor(/ready: app mounted in \d+ ms, \d+ ms after the edit; restored: count/, 300000);
    fs.writeFileSync(path.join(root, 'src', 'App.vue'), fs.readFileSync(path.join(root, 'src', 'App.vue'), 'utf8').replace('<h1>State</h1>', '<h1>State two</h1>'));
    await edited;
    const exited = new Promise((resolve) => child.once('close', (code, signal) => resolve({ code, signal })));
    child.kill('SIGINT');
    const { code, signal } = await exited;
    assert.equal(signal, null);
    assert.equal(code, 0, stderr);
    assert.match(stdout, /vue-native dev: stopped\n$/);
  });
});

// A writable computed ref is derived from other state: the snapshot leaves it out and the restart recomputes it. Its setter
// throws (tests/cli/fixtures/dev-computed), so a restore that assigned it would fail the restart. The negative control makes
// the same binding a plain ref, which is saved and restored.
describe('vue-native dev: a computed ref is derived, not saved', () => {
  test('a writable computed is left out of the snapshot and recomputed from the restored count', async () => {
    const root = copyProject(path.join(FIXTURES, 'dev-computed'), 'computed');
    const { session } = start(root);
    await session.started;
    await session.steps(clicks('#increment', 3));
    const cycle = session.nextIdle();
    editFile(root, 'src/App.vue', '<h1>Computed</h1>', '<h1>Computed, edited</h1>');
    assert.equal((await cycle).outcome, 'ready');
    const after = await session.steps([{ op: 'snapshot', name: 'after' }, { op: 'html', selector: '#count' }, { op: 'html', selector: '#doubled' }]);
    assert.match(htmlOf(after, 0), /Computed, edited/, 'the edit is in the running app');
    assert.match(htmlOf(after, 1), />Count: 3<\/p>$/, 'the count is restored');
    assert.match(htmlOf(after, 2), />Doubled: 6<\/p>$/, 'the computed is recomputed from the restored count');
    assert.deepEqual(lastEvent(session, 'ready').restored, ['count']);
    assert.deepEqual(lastEvent(session, 'ready').skipped, []);
  });

  test('negative control: the same binding as a plain ref is saved and restored', async () => {
    const root = copyProject(path.join(FIXTURES, 'dev-computed'), 'computed-plain');
    editFile(root, 'src/App.vue', "const doubled = computed({\n  get: () => count.value * 2,\n  set: () => { throw new Error('the setter must not run on restore') },\n})", 'const doubled = ref(0)');
    const { session } = start(root);
    await session.started;
    await session.steps(clicks('#increment', 3));
    const cycle = session.nextIdle();
    editFile(root, 'src/App.vue', '<h1>Computed</h1>', '<h1>Computed, edited</h1>');
    assert.equal((await cycle).outcome, 'ready');
    assert.deepEqual(lastEvent(session, 'ready').restored, ['count', 'doubled']);
  });
});
