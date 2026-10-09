// WP-G0A: the prototype fixtures are immutable. MANIFEST.json pins every preserved file by SHA-256 and size.
// Run: node --test tests/fixtures/prototype/verify-fixtures.test.mjs
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {existsSync, lstatSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, symlinkSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join, relative, sep} from 'node:path';
import {fileURLToPath} from 'node:url';

const ROOT = dirname(fileURLToPath(import.meta.url));
// Harness files at the prototype root. They are not preserved fixtures, so they are exempt from the extra-file check.
const HARNESS_FILES = new Set(['README.md', 'MANIFEST.json', 'verify-fixtures.test.mjs', 'regressions.test.mjs']);
const ORIGIN_BY_DIR = new Map([['counter', 'workspace'], ['quick-quote', 'zip']]);
// Stated in planning/vue-native-framework/requirements.md (the workspace compiler SHA-256 claim).
const CLAIMED_WORKSPACE_SHA256 = '24224cdbcc536bfcbaa970bd4595190c974bc26998fc3f8d814685064d31afad';

const sha256 = buf => createHash('sha256').update(buf).digest('hex');
const loadManifest = dir => JSON.parse(readFileSync(join(dir, 'MANIFEST.json'), 'utf8'));

// Lists every entry under dir as a posix relative path. Symlinks and other non-regular entries are reported as such.
function listTree(dir) {
  const out = [];
  const visit = abs => {
    for (const name of readdirSync(abs).sort()) {
      const p = join(abs, name);
      const st = lstatSync(p);
      const rel = relative(dir, p).split(sep).join('/');
      if (st.isDirectory()) visit(p);
      else if (st.isFile()) out.push({rel, kind: 'file'});
      else out.push({rel, kind: st.isSymbolicLink() ? 'symlink' : 'other'});
    }
  };
  visit(dir);
  return out;
}

// The verification function under test. Returns a list of problems; an empty list means the tree matches the manifest.
export function verifyFixtureTree(dir, manifest) {
  const problems = [];
  const listed = new Set();
  for (const f of manifest.files) {
    const top = f.path.split('/')[0];
    const unsafe = f.path.startsWith('/') || f.path.includes('\\') || f.path.split('/').includes('..');
    if (unsafe || !ORIGIN_BY_DIR.has(top)) {
      problems.push(`bad manifest path: ${f.path}`);
      continue;
    }
    if (f.origin !== ORIGIN_BY_DIR.get(top)) problems.push(`origin mismatch: ${f.path} is ${f.origin}`);
    if (listed.has(f.path)) problems.push(`duplicate manifest path: ${f.path}`);
    listed.add(f.path);
    const abs = join(dir, f.path);
    if (!existsSync(abs) || !lstatSync(abs).isFile()) {
      problems.push(`missing: ${f.path}`);
      continue;
    }
    const buf = readFileSync(abs);
    if (buf.length !== f.bytes) problems.push(`size mismatch: ${f.path} manifest ${f.bytes} actual ${buf.length}`);
    if (sha256(buf) !== f.sha256) problems.push(`sha256 mismatch: ${f.path}`);
  }
  for (const e of listTree(dir)) {
    if (HARNESS_FILES.has(e.rel)) continue;
    if (e.kind !== 'file') problems.push(`not a regular file: ${e.rel}`);
    else if (!listed.has(e.rel)) problems.push(`extra file not in manifest: ${e.rel}`);
  }
  return problems;
}

// Builds a temporary tree from {relativePath: text}, with a manifest that pins exactly those files, then runs
// fn(dir, manifest). Harness files (for the exemption control) are written without being pinned.
function withTempTree(files, fn, harnessFiles = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'vn-g0a-verify-'));
  try {
    const entries = [];
    for (const [rel, text] of Object.entries(files)) {
      const abs = join(dir, rel);
      mkdirSync(dirname(abs), {recursive: true});
      writeFileSync(abs, text);
      const buf = readFileSync(abs);
      entries.push({path: rel, sha256: sha256(buf), bytes: buf.length, origin: ORIGIN_BY_DIR.get(rel.split('/')[0])});
    }
    for (const [rel, text] of Object.entries(harnessFiles)) writeFileSync(join(dir, rel), text);
    return fn(dir, {files: entries});
  } finally {
    rmSync(dir, {recursive: true, force: true});
  }
}

const TEMP_FILES = {
  'counter/compiler/a.mjs': 'export const v = "one";\n',
  'quick-quote/tests/b.test.mjs': 'import {test} from "node:test";\n',
};

test('MANIFEST pins every preserved fixture file: sha256, size and origin all match', () => {
  assert.deepEqual(verifyFixtureTree(ROOT, loadManifest(ROOT)), []);
});

test('MANIFEST lists files sorted by path, without duplicates', () => {
  const paths = loadManifest(ROOT).files.map(f => f.path);
  assert.deepEqual(paths, [...paths].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0)));
  assert.equal(new Set(paths).size, paths.length);
});

test('workspace compiler hash in MANIFEST equals the claimed value and the copied counter compiler', () => {
  assert.equal(loadManifest(ROOT).workspaceCompilerSha256, CLAIMED_WORKSPACE_SHA256);
  assert.equal(sha256(readFileSync(join(ROOT, 'counter/compiler/compile.mjs'))), CLAIMED_WORKSPACE_SHA256);
});

test('zip compiler hash in MANIFEST equals the copied quick-quote compiler', () => {
  const m = loadManifest(ROOT);
  assert.equal(sha256(readFileSync(join(ROOT, 'quick-quote/compiler/compile.mjs'))), m.zipCompilerSha256);
});

// The archive lives outside the repository. When it is absent these two tests are reported as skipped, not passed.
const zipPath = loadManifest(ROOT).zip.path;
const zipSkip = existsSync(zipPath) ? false : `source archive not present at ${zipPath}`;

test('Quick Quote archive matches its recorded SHA-256 and size', {skip: zipSkip}, () => {
  const m = loadManifest(ROOT);
  const buf = readFileSync(zipPath);
  assert.equal(buf.length, m.zip.bytes);
  assert.equal(sha256(buf), m.zip.sha256);
});

test('quick-quote fixture files are byte-identical to their archive entries', {skip: zipSkip}, () => {
  for (const f of loadManifest(ROOT).files.filter(x => x.origin === 'zip')) {
    // Reads one entry to stdout. Nothing is extracted to disk and nothing is executed.
    const r = spawnSync('unzip', ['-p', zipPath, f.path], {maxBuffer: 64 * 1024 * 1024});
    assert.equal(r.status, 0, `unzip could not read ${f.path}: ${String(r.stderr)}`);
    assert.equal(sha256(r.stdout), f.sha256, `archive entry differs from fixture file: ${f.path}`);
  }
});

test('negative control: the checker reports nothing for an untouched temporary tree', () => {
  withTempTree(TEMP_FILES, (dir, m) => {
    assert.deepEqual(verifyFixtureTree(dir, m), []);
  });
});

test('negative control: a one-byte change is detected (sha256 mismatch, size unchanged)', () => {
  withTempTree(TEMP_FILES, (dir, m) => {
    const abs = join(dir, 'counter/compiler/a.mjs');
    const buf = readFileSync(abs);
    buf[buf.indexOf('one')] ^= 0x01; // 'o' -> 'n': one byte, same length
    writeFileSync(abs, buf);
    assert.deepEqual(verifyFixtureTree(dir, m), ['sha256 mismatch: counter/compiler/a.mjs']);
  });
});

test('negative control: an appended byte is detected (size and sha256 mismatch)', () => {
  withTempTree(TEMP_FILES, (dir, m) => {
    writeFileSync(join(dir, 'counter/compiler/a.mjs'), TEMP_FILES['counter/compiler/a.mjs'] + '\n');
    assert.deepEqual(verifyFixtureTree(dir, m), [
      'size mismatch: counter/compiler/a.mjs manifest 24 actual 25',
      'sha256 mismatch: counter/compiler/a.mjs',
    ]);
  });
});

test('negative control: an extra file under a fixture directory is detected', () => {
  withTempTree(TEMP_FILES, (dir, m) => {
    writeFileSync(join(dir, 'counter/compiler/extra.mjs'), 'export {};\n');
    assert.deepEqual(verifyFixtureTree(dir, m), ['extra file not in manifest: counter/compiler/extra.mjs']);
  });
});

test('negative control: a missing manifest file is detected', () => {
  withTempTree(TEMP_FILES, (dir, m) => {
    rmSync(join(dir, 'quick-quote/tests/b.test.mjs'));
    assert.deepEqual(verifyFixtureTree(dir, m), ['missing: quick-quote/tests/b.test.mjs']);
  });
});

test('negative control: a symlink inside a fixture directory is detected', () => {
  withTempTree(TEMP_FILES, (dir, m) => {
    symlinkSync(join(dir, 'counter/compiler/a.mjs'), join(dir, 'counter/compiler/link.mjs'));
    assert.deepEqual(verifyFixtureTree(dir, m), ['not a regular file: counter/compiler/link.mjs']);
  });
});

test('negative control: a manifest path that escapes the fixture is rejected', () => {
  withTempTree(TEMP_FILES, (dir, m) => {
    m.files.push({path: 'counter/../../escape.mjs', sha256: '0', bytes: 0, origin: 'workspace'});
    assert.deepEqual(verifyFixtureTree(dir, m), ['bad manifest path: counter/../../escape.mjs']);
  });
});

test('negative control: harness files at the prototype root are exempt from the extra-file check', () => {
  withTempTree(TEMP_FILES, (dir, m) => {
    assert.deepEqual(verifyFixtureTree(dir, m), []);
  }, {'README.md': '# provenance\n'});
});

test('negative control: a harness-named file inside a fixture directory is not exempt', () => {
  withTempTree(TEMP_FILES, (dir, m) => {
    writeFileSync(join(dir, 'counter/README.md'), '# not a fixture\n');
    assert.deepEqual(verifyFixtureTree(dir, m), ['extra file not in manifest: counter/README.md']);
  });
});
