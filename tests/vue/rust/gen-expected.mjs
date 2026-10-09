// Oracle for the WP-R1a Rust tests (crates/reactivity/tests). Runs every snippet in snippets.json against the real
// @vue/reactivity 3.5.43 production build (NODE_ENV=production) and records what each snippet logs.
//   node tests/vue/rust/gen-expected.mjs            regenerate expected.json (deterministic bytes)
//   node tests/vue/rust/gen-expected.mjs --check    exit 1 when expected.json differs from a fresh run
// Each snippet runs in its own child process, so no module state leaks between snippets. A snippet that throws
// without catching is a generator error (the Rust side expects every outcome to be logged).
import {spawnSync} from 'node:child_process';
import {readFileSync, writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const self = fileURLToPath(import.meta.url);
const snippetsFile = path.join(here, 'snippets.json');
const expectedFile = path.join(here, 'expected.json');
const require = createRequire(import.meta.url);

if (process.argv[2] === '--child') {
  const index = Number(process.argv[3]);
  const snippet = JSON.parse(readFileSync(snippetsFile, 'utf8'))[index];
  const R = require('@vue/reactivity');
  const loaded = Object.keys(require.cache).filter((k) => k.endsWith('reactivity.cjs.prod.js'));
  if (process.env.NODE_ENV !== 'production' || loaded.length !== 1) {
    throw new Error('oracle must load the production build of @vue/reactivity');
  }
  const logs = [];
  const log = (...args) => {
    logs.push(args.map((a) => String(a)).join(' '));
  };
  new Function('R', 'log', "'use strict';\n" + snippet.lines.join('\n'))(R, log);
  process.stdout.write(JSON.stringify(logs));
} else {
  const snippets = JSON.parse(readFileSync(snippetsFile, 'utf8'));
  const ids = new Set();
  for (const s of snippets) {
    if (ids.has(s.id)) throw new Error(`duplicate snippet id ${s.id}`);
    ids.add(s.id);
  }
  const version = require('@vue/reactivity/package.json').version;
  if (version !== '3.5.43') throw new Error(`expected @vue/reactivity 3.5.43, found ${version}`);
  const cases = {};
  for (let i = 0; i < snippets.length; i++) {
    const r = spawnSync(process.execPath, [self, '--child', String(i)], {
      encoding: 'utf8',
      env: {...process.env, NODE_ENV: 'production'},
    });
    if (r.status !== 0) {
      process.stderr.write(`snippet ${snippets[i].id} failed:\n${r.stderr}`);
      process.exit(1);
    }
    cases[snippets[i].id] = JSON.parse(r.stdout);
  }
  const text = JSON.stringify(
    {oracle: `@vue/reactivity ${version} production build (NODE_ENV=production)`, cases},
    null,
    2,
  ) + '\n';
  if (process.argv[2] === '--check') {
    const current = readFileSync(expectedFile, 'utf8');
    if (current !== text) {
      process.stderr.write('expected.json is stale: run node tests/vue/rust/gen-expected.mjs\n');
      process.exit(1);
    }
    process.stdout.write(`expected.json matches a fresh run (${snippets.length} snippets)\n`);
  } else {
    writeFileSync(expectedFile, text);
    process.stdout.write(`wrote ${expectedFile} (${snippets.length} snippets)\n`);
  }
}
