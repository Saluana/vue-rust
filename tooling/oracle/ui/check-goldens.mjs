// Golden check for UI scenarios (WP-V1).
//   node tooling/oracle/ui/check-goldens.mjs [--update] [--filter <text>]
// Golden of tests/ui/cases/<group>/<name>/ is tests/ui/golden/<group>/<name>.json: one canonical line (the document),
// newline-terminated. --update rewrites the goldens and removes orphans; --filter limits the run to matching names.
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { compareDocs, describeDiff } from '../compare.mjs';
import { listUiCases, runUiScenario, uiCasesRoot, uiGoldenRoot, uiScenarioName } from './run-ui.mjs';

export function goldenPathFor(dir, root = uiGoldenRoot) {
  return path.join(root, `${uiScenarioName(dir)}.json`);
}

// Canonical line of a document: JSON with no spaces, newline-terminated.
export function canonicalLine(doc) {
  return `${JSON.stringify(doc)}\n`;
}

// null when the actual line matches the golden exactly (structurally and byte for byte), else the reason.
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
  if (actualLine !== goldenText) return 'bytes differ from the canonical line';
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

async function listGoldens(root) {
  const out = [];
  const walk = async (dir) => {
    const entries = await readdir(dir, { withFileTypes: true });
    entries.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
    for (const ent of entries) {
      const p = path.join(dir, ent.name);
      if (ent.isDirectory()) await walk(p);
      else if (ent.isFile() && ent.name.endsWith('.json')) out.push(p);
    }
  };
  try {
    await walk(root);
  } catch (e) {
    if (e.code !== 'ENOENT') throw e;
  }
  return out;
}

async function mapLimit(items, limit, fn) {
  const results = new Array(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const i = next++;
      results[i] = await fn(items[i]);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}

// Returns { results: [{ dir, scenario, status: 'ok' | 'updated' | 'fail', reason? }], orphans: [golden paths without a case] }.
export async function checkUiGoldens({
  update = false, filter = null, casesRoot = uiCasesRoot, goldenRoot = uiGoldenRoot, concurrency = 4,
} = {}) {
  const all = listUiCases(casesRoot);
  const cases = filter === null ? all : all.filter((d) => uiScenarioName(d).includes(filter));
  const results = await mapLimit(cases, concurrency, async (dir) => {
    const scenario = uiScenarioName(dir);
    const gfile = goldenPathFor(dir, goldenRoot);
    let line;
    try {
      line = canonicalLine(await runUiScenario(dir));
    } catch (e) {
      return { dir, scenario, status: 'fail', reason: `scenario failed to run: ${String(e.message).split('\n')[0]}` };
    }
    if (update) {
      await mkdir(path.dirname(gfile), { recursive: true });
      await writeFile(gfile, line);
      return { dir, scenario, status: 'updated' };
    }
    const reason = compareWithGolden(line, await readOrNull(gfile));
    return reason === null ? { dir, scenario, status: 'ok' } : { dir, scenario, status: 'fail', reason };
  });
  let orphans = [];
  if (filter === null) {
    const expected = new Set(all.map((d) => goldenPathFor(d, goldenRoot)));
    orphans = (await listGoldens(goldenRoot)).filter((g) => !expected.has(g));
    if (update) for (const g of orphans) await rm(g);
  }
  return { results, orphans };
}

async function main(argv) {
  const update = argv.includes('--update');
  const fi = argv.indexOf('--filter');
  const filter = fi >= 0 ? argv[fi + 1] ?? null : null;
  const { results, orphans } = await checkUiGoldens({ update, filter });
  const rel = (f) => path.relative(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..'), f).split(path.sep).join('/');
  let failures = 0;
  for (const r of results) {
    if (r.status !== 'fail') continue;
    failures += 1;
    process.stdout.write(`FAIL ${r.scenario}: ${r.reason}\n`);
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
