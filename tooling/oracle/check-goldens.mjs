// Golden check for language scenarios.
//   node tooling/oracle/check-goldens.mjs           check every case against its golden (exit 1 on any mismatch)
//   node tooling/oracle/check-goldens.mjs --update  rewrite goldens with the exact CLI output bytes
// Golden of tests/language/cases/<rel>.js is tests/language/golden/<rel>.js.json (one line, newline-terminated).
import {mkdir, readdir, readFile, rm, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {compareDocs, describeDiff} from './compare.mjs';
import {casesRoot, runScenarioLine} from './node-runner.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
export const goldenRoot = path.resolve(here, '../../tests/language/golden');

async function listFiles(root, suffix) {
  const out = [];
  async function walkDir(dir) {
    const entries = await readdir(dir, {withFileTypes: true});
    entries.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
    for (const ent of entries) {
      const p = path.join(dir, ent.name);
      if (ent.isDirectory()) await walkDir(p);
      else if (ent.isFile() && ent.name.endsWith(suffix)) out.push(p);
    }
  }
  try {
    await walkDir(root);
  } catch (e) {
    if (e.code !== 'ENOENT') throw e;
  }
  return out;
}

// All scenario files under root (*.js), in a deterministic order. README.md is not a case.
export function listCases(root = casesRoot) {
  return listFiles(root, '.js');
}

export function goldenPathFor(caseFile, root = goldenRoot, cases = casesRoot) {
  const rel = path.relative(cases, caseFile).split(path.sep).join('/');
  return path.join(root, `${rel}.json`);
}

// null when the actual output matches the golden exactly (structurally and byte for byte), else a reason.
export function compareWithGolden(actualLine, goldenText) {
  if (goldenText === null) return 'missing golden (run with --update)';
  let actual;
  let golden;
  try {
    actual = JSON.parse(actualLine);
  } catch {
    return 'actual output is not JSON';
  }
  try {
    golden = JSON.parse(goldenText);
  } catch {
    return 'golden is not JSON';
  }
  const d = compareDocs(actual, golden);
  if (d !== null) return `${describeDiff(d)} (actual vs golden)`;
  if (actualLine !== goldenText) return 'bytes differ from the canonical CLI output';
  return null;
}

async function readOrNull(file) {
  try {
    return await readFile(file, 'utf8');
  } catch (e) {
    if (e.code === 'ENOENT') return null;
    throw e;
  }
}

async function mapLimit(items, limit, fn) {
  const results = new Array(items.length);
  let next = 0;
  async function worker() {
    while (next < items.length) {
      const i = next++;
      results[i] = await fn(items[i]);
    }
  }
  await Promise.all(Array.from({length: Math.min(limit, items.length)}, worker));
  return results;
}

// Returns {results: [{file, status: 'ok'|'updated'|'fail', reason?}], orphans: [golden paths without a case]}.
// In update mode orphan goldens are removed and reported in `orphans`.
export async function checkAll({update = false} = {}) {
  const cases = await listCases(casesRoot);
  const results = await mapLimit(cases, 4, async (file) => {
    const gfile = goldenPathFor(file);
    let line;
    try {
      line = await runScenarioLine(file);
    } catch (e) {
      return {file, status: 'fail', reason: `scenario failed to run: ${e.message.split('\n')[0]}`};
    }
    if (update) {
      await mkdir(path.dirname(gfile), {recursive: true});
      await writeFile(gfile, line);
      return {file, status: 'updated'};
    }
    const reason = compareWithGolden(line, await readOrNull(gfile));
    return reason === null ? {file, status: 'ok'} : {file, status: 'fail', reason};
  });
  const expected = new Set(cases.map((c) => goldenPathFor(c)));
  const orphans = (await listFiles(goldenRoot, '.json')).filter((g) => !expected.has(g));
  if (update) for (const g of orphans) await rm(g);
  return {results, orphans};
}

async function main(argv) {
  const update = argv.includes('--update');
  const {results, orphans} = await checkAll({update});
  const rel = (f) => path.relative(path.resolve(here, '../..'), f).split(path.sep).join('/');
  let failures = 0;
  for (const r of results) {
    if (r.status !== 'fail') continue;
    failures += 1;
    process.stdout.write(`FAIL ${rel(r.file)}: ${r.reason}\n`);
  }
  for (const g of orphans) {
    if (!update) failures += 1;
    process.stdout.write(`${update ? 'removed' : 'ORPHAN'} golden ${rel(g)}\n`);
  }
  const good = results.filter((r) => r.status === 'ok' || r.status === 'updated').length;
  process.stdout.write(update
    ? `updated ${good} golden(s) of ${results.length} scenario(s)\n`
    : `${good}/${results.length} scenario(s) match their goldens\n`);
  process.exitCode = failures === 0 ? 0 : 1;
}

const isMain = process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) await main(process.argv.slice(2));
