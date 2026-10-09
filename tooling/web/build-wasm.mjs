#!/usr/bin/env node
// Builds the browser host (WP-W1a). Produces, for each variant, a directory with the content-hashed wasm, the glue, the page
// shell and manifest.json: { abiVersion, wasm, hash, bytes, glue, features, profile, target }. The "selftest" variant adds
// the JSON self-test export used by tests/platform/web; the shipped variant does not contain it.
//   node tooling/web/build-wasm.mjs [--out <dir>] [--only shipped|selftest]
// The output directory defaults to $CARGO_TARGET_DIR/web-dist (or target/shared/web-dist). Requires the project toolchain
// (scripts/env.sh); the PATH and rustup/cargo homes are set here when the caller has not sourced it.
import { execFileSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';

export const ROOT = fileURLToPath(new URL('../../', import.meta.url));
export const ABI_VERSION = 1;
const WASM_NAME = 'vn_renderer_dom.wasm';

export function cargoEnv() {
  const env = { ...process.env, CARGO_INCREMENTAL: '0' };
  const toolchain = path.join(ROOT, 'toolchain');
  if (fs.existsSync(path.join(toolchain, 'cargo', 'bin'))) {
    env.RUSTUP_HOME ??= path.join(toolchain, 'rustup');
    env.CARGO_HOME ??= path.join(toolchain, 'cargo');
    env.PATH = `${env.CARGO_HOME}/bin:${env.PATH}`;
  }
  env.CARGO_TARGET_DIR ??= path.join(ROOT, 'target', 'shared');
  return env;
}

export function defaultOutDir() {
  return path.join(cargoEnv().CARGO_TARGET_DIR, 'web-dist');
}

// Builds one variant with cargo and returns the manifest. `selftest` adds the feature of the same name.
export function buildVariant(outDir, { selftest }) {
  const env = cargoEnv();
  const args = ['build', '--release', '--target', 'wasm32-unknown-unknown', '-p', 'vn-renderer-dom'];
  if (selftest) args.push('--features', 'selftest');
  execFileSync('cargo', args, { cwd: ROOT, env, stdio: ['ignore', 'ignore', 'pipe'] });
  const built = path.join(env.CARGO_TARGET_DIR, 'wasm32-unknown-unknown', 'release', WASM_NAME);
  const bytes = fs.readFileSync(built);
  const hash = crypto.createHash('sha256').update(bytes).digest('hex');
  const dir = selftest ? path.join(outDir, 'selftest') : path.join(outDir, 'shipped');
  fs.mkdirSync(dir, { recursive: true });
  const wasmName = `vn_renderer_dom-${hash.slice(0, 16)}.wasm`;
  for (const old of fs.readdirSync(dir)) {
    if (/^vn_renderer_dom-[0-9a-f]{16}\.wasm$/.test(old) && old !== wasmName) fs.rmSync(path.join(dir, old));
  }
  fs.writeFileSync(path.join(dir, wasmName), bytes);
  fs.copyFileSync(path.join(ROOT, 'hosts', 'web', 'vn-host.js'), path.join(dir, 'vn-host.js'));
  fs.copyFileSync(path.join(ROOT, 'hosts', 'web', 'index.html'), path.join(dir, 'index.html'));
  const manifest = {
    abiVersion: ABI_VERSION,
    wasm: wasmName,
    hash,
    bytes: bytes.length,
    glue: 'vn-host.js',
    features: selftest ? ['selftest'] : [],
    profile: 'release',
    target: 'wasm32-unknown-unknown',
  };
  fs.writeFileSync(path.join(dir, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  return { dir, manifest, gzipBytes: zlib.gzipSync(bytes, { level: 9 }).length };
}

export function buildWebDist({ outDir = defaultOutDir(), only = 'both' } = {}) {
  const variants = [];
  if (only !== 'selftest') variants.push(buildVariant(outDir, { selftest: false }));
  if (only !== 'shipped') variants.push(buildVariant(outDir, { selftest: true }));
  return variants;
}

function main(argv) {
  const outIndex = argv.indexOf('--out');
  const outDir = outIndex >= 0 ? path.resolve(argv[outIndex + 1]) : defaultOutDir();
  const onlyIndex = argv.indexOf('--only');
  const only = onlyIndex >= 0 ? argv[onlyIndex + 1] : 'both';
  if (!['both', 'shipped', 'selftest'].includes(only)) throw new Error(`--only must be shipped or selftest, not ${only}`);
  for (const { dir, manifest, gzipBytes } of buildWebDist({ outDir, only })) {
    const kib = (manifest.bytes / 1024).toFixed(1);
    const label = manifest.features.length ? 'selftest' : 'shipped';
    process.stdout.write(
      `${label}: ${manifest.wasm} ${manifest.bytes} bytes (${kib} KiB, gzip ${gzipBytes} bytes) in ${dir}\n`,
    );
  }
  process.stdout.write(`manifest: ${path.join(outDir, 'shipped', 'manifest.json')}\n`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (err) {
    process.stderr.write(`build-wasm: ${err.message}\n`);
    if (err.stderr) process.stderr.write(String(err.stderr));
    process.exitCode = 1;
  }
}
