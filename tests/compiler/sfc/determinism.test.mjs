// Determinism (WP-F1): compiling a fixture twice, in another process, and from another working directory gives the same
// bytes. Outputs contain no timestamps and no absolute paths, so nothing in them depends on where the project lives.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { FIXTURES, ROOT } from './harness.mjs';

const MODULE = fileURLToPath(new URL('../../../compiler/sfc/index.mjs', import.meta.url));
const FILES = fs.readdirSync(FIXTURES).filter((f) => f.endsWith('.vue')).sort();

// JSON with sorted keys, so that equal values compare equal however their objects were built.
function stable(value) {
  return JSON.stringify(value, (key, v) => (v && typeof v === 'object' && !Array.isArray(v)
    ? Object.fromEntries(Object.keys(v).sort().map((k) => [k, v[k]]))
    : v));
}

// Every field of a result, with diagnostics as plain objects.
function snapshot(r) {
  return stable({
    code: r.code,
    map: r.map,
    styles: r.styles,
    bindings: r.bindings,
    diagnostics: r.diagnostics.map((d) => ({ code: d.code, severity: d.severity, file: d.file, start: d.start, end: d.end, message: d.message, hint: d.hint })),
    dependencies: r.dependencies,
    scopeId: r.scopeId,
    lang: r.lang,
    sources: r.sources,
  });
}

function compileIn(file, rootDir, cwdDir) {
  const script = `
    import fs from 'node:fs';
    import { compileSfc } from ${JSON.stringify(MODULE)};
    const stable = ${stable.toString()};
    const snapshot = ${snapshot.toString()};
    const SNAPSHOT = snapshot;
    const out = {};
    for (const f of ${JSON.stringify(FILES)}) {
      const src = fs.readFileSync(${JSON.stringify(FIXTURES)} + '/' + f, 'utf8');
      const r = compileSfc(src, { filename: f, rootDir: ${JSON.stringify(rootDir)} });
      out[f] = SNAPSHOT(r);
    }
    process.stdout.write(JSON.stringify(out));`;
  const stdout = execFileSync(process.execPath, ['--input-type=module', '--eval', script], {
    cwd: cwdDir,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  return JSON.parse(stdout);
}

describe('determinism', () => {
  test('compiling each fixture twice in one process gives identical results', async () => {
    const { compileSfc } = await import(MODULE);
    for (const f of FILES) {
      const src = fs.readFileSync(path.join(FIXTURES, f), 'utf8');
      const a = snapshot(compileSfc(src, { filename: f, rootDir: FIXTURES }));
      const b = snapshot(compileSfc(src, { filename: f, rootDir: FIXTURES }));
      assert.equal(a, b, `${f} is stable across two compiles`);
    }
  });

  test('a different working directory and a relative rootDir give byte-identical output', async () => {
    const { compileSfc } = await import(MODULE);
    const here = {};
    for (const f of FILES) {
      const src = fs.readFileSync(path.join(FIXTURES, f), 'utf8');
      here[f] = snapshot(compileSfc(src, { filename: f, rootDir: FIXTURES }));
    }
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'vn-sfc-cwd-'));
    try {
      const fromTmp = compileIn(FILES[0], FIXTURES, tmp);
      const fromFixtures = compileIn(FILES[0], '.', FIXTURES);
      for (const f of FILES) {
        assert.equal(fromTmp[f], here[f], `${f}: other working directory equals in-process output`);
        assert.equal(fromFixtures[f], here[f], `${f}: relative rootDir equals in-process output`);
      }
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  test('no output mentions an absolute path of this machine', async () => {
    const { compileSfc } = await import(MODULE);
    for (const f of FILES) {
      const src = fs.readFileSync(path.join(FIXTURES, f), 'utf8');
      const r = compileSfc(src, { filename: f, rootDir: FIXTURES });
      const text = JSON.stringify({ code: r.code, map: r.map, styles: r.styles, diagnostics: r.diagnostics, sources: r.sources, dependencies: r.dependencies });
      assert.ok(!text.includes(ROOT), `${f}: no repository path in output`);
      assert.ok(!text.includes(os.homedir()), `${f}: no home directory in output`);
    }
  });
});
