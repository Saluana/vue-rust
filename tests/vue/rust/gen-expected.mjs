// Oracle for the Rust reactivity tests (crates/reactivity/tests). Each group pairs a snippet list with the expected.json
// that the Rust tests read. Snippets run against the real @vue/reactivity 3.5.43 production build (NODE_ENV=production)
// and record what each one logs. Group 0 is WP-R1a (snippets.json); group 1 is WP-R1b (reactive/snippets.mjs, whose
// default export is the list; a snippet has `code`, or `lines`).
//   node tests/vue/rust/gen-expected.mjs            regenerate every expected.json (deterministic bytes)
//   node tests/vue/rust/gen-expected.mjs --check    exit 1 when an expected.json differs from a fresh run
// Each snippet runs in its own child process, so no module state leaks between snippets. A snippet that throws
// without catching is a generator error (the Rust side expects every outcome to be logged).
import {spawnSync} from 'node:child_process';
import {readFileSync, writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const self = fileURLToPath(import.meta.url);
const require = createRequire(import.meta.url);

const groups = [
  {snippets: path.join(here, 'snippets.json'), expected: path.join(here, 'expected.json')},
  {snippets: path.join(here, 'reactive', 'snippets.mjs'), expected: path.join(here, 'reactive', 'expected.json')},
];

async function loadSnippets(file) {
  if (file.endsWith('.mjs')) return (await import(pathToFileURL(file).href)).default;
  return JSON.parse(readFileSync(file, 'utf8'));
}

function sourceOf(snippet) {
  return snippet.code ?? snippet.lines.join('\n');
}

if (process.argv[2] === '--child') {
  const group = Number(process.argv[3]);
  const index = Number(process.argv[4]);
  const snippet = (await loadSnippets(groups[group].snippets))[index];
  const R = require('@vue/reactivity');
  const loaded = Object.keys(require.cache).filter((k) => k.endsWith('reactivity.cjs.prod.js'));
  if (process.env.NODE_ENV !== 'production' || loaded.length !== 1) {
    throw new Error('oracle must load the production build of @vue/reactivity');
  }
  const logs = [];
  const log = (...args) => {
    logs.push(args.map((a) => String(a)).join(' '));
  };
  new Function('R', 'log', "'use strict';\n" + sourceOf(snippet))(R, log);
  process.stdout.write(JSON.stringify(logs));
} else {
  const version = require('@vue/reactivity/package.json').version;
  if (version !== '3.5.43') throw new Error(`expected @vue/reactivity 3.5.43, found ${version}`);
  const checking = process.argv[2] === '--check';
  for (let g = 0; g < groups.length; g++) {
    const snippets = await loadSnippets(groups[g].snippets);
    const ids = new Set();
    for (const s of snippets) {
      if (ids.has(s.id)) throw new Error(`duplicate snippet id ${s.id}`);
      ids.add(s.id);
    }
    const cases = {};
    for (let i = 0; i < snippets.length; i++) {
      const r = spawnSync(process.execPath, [self, '--child', String(g), String(i)], {
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
    const name = path.relative(here, groups[g].expected);
    if (checking) {
      if (readFileSync(groups[g].expected, 'utf8') !== text) {
        process.stderr.write(`${name} is stale: run node tests/vue/rust/gen-expected.mjs\n`);
        process.exit(1);
      }
      process.stdout.write(`${name} matches a fresh run (${snippets.length} snippets)\n`);
    } else {
      writeFileSync(groups[g].expected, text);
      process.stdout.write(`wrote ${name} (${snippets.length} snippets)\n`);
    }
  }
}
