// Shared helpers for the build-isolation tests (WP-G0C). Not a test file: the glob in the acceptance command
// only matches *.test.mjs.
import { spawn, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { SANDBOX_EXEC, sandboxAvailability } from '../../../tooling/sandbox/run-untrusted.mjs';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
export const RUNNER = path.join(ROOT, 'tooling/sandbox/run-untrusted.mjs');
export const FIXTURES = path.join(ROOT, 'tests/security/build-isolation/fixtures');
export const RUSTC = path.join(ROOT, 'toolchain/cargo/bin/rustc');

export const availability = sandboxAvailability({ sandboxExec: SANDBOX_EXEC });
export const NO_SANDBOX_SKIP = availability.available
  ? false
  : `LOUD SKIP: ${SANDBOX_EXEC} unavailable (${availability.reason}). Sandboxed isolation is NOT verified by this run.`;
if (NO_SANDBOX_SKIP) process.stderr.write(`\n*** WP-G0C: ${NO_SANDBOX_SKIP}\n`);

export const RUST_SKIP = fs.existsSync(RUSTC)
  ? false
  : `LOUD SKIP: project rustc not found at ${RUSTC}. The Rust build-script fixture did not run.`;
if (RUST_SKIP) process.stderr.write(`\n*** WP-G0C: ${RUST_SKIP}\n`);

// A disposable tree: a fake home holding a sentinel secret, a workspace, a scratch dir, and an "outside" dir
// inside the fake home. Everything is under the real temp directory and removed by cleanup().
export function makeRoot() {
  const root = fs.mkdtempSync(path.join(fs.realpathSync(os.tmpdir()), 'vn-g0c-'));
  const home = path.join(root, 'home');
  const ws = path.join(root, 'ws');
  const scratch = path.join(root, 'scratch');
  const outside = path.join(home, 'outside');
  for (const d of [home, ws, scratch, outside]) fs.mkdirSync(d);
  const token = `VN-G0C-SENTINEL-${crypto.randomBytes(8).toString('hex')}`;
  const nonce = `VN-G0C-NONCE-${crypto.randomBytes(8).toString('hex')}`;
  fs.writeFileSync(path.join(home, '.vn-g0c-secret'), `${token}\n`, { mode: 0o600 });
  fs.writeFileSync(path.join(ws, 'input.txt'), 'WORKSPACE-INPUT\n');
  return {
    root,
    home,
    ws,
    scratch,
    outside,
    token,
    nonce,
    secretPath: path.join(home, '.vn-g0c-secret'),
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

// The environment a fixture sees. Same for sandboxed and unsandboxed runs, so only the sandbox differs.
export function fixtureEnv(t, port, { outsideName = 'planted.txt' } = {}) {
  return {
    HOME: t.home,
    TMPDIR: t.scratch,
    PATH: '/usr/bin:/bin:/usr/sbin:/sbin',
    VN_G0C_WORKSPACE: t.ws,
    VN_G0C_OUTSIDE: path.join(t.outside, outsideName),
    VN_G0C_PORT: String(port),
    VN_G0C_NONCE: t.nonce,
  };
}

// Localhost TCP listener the tests open. conns records every accepted connection and what it received.
export function startListener() {
  const conns = [];
  const socks = new Set();
  const server = net.createServer((sock) => {
    const rec = { data: '' };
    conns.push(rec);
    socks.add(sock);
    sock.setEncoding('utf8');
    sock.on('data', (d) => {
      rec.data += d;
    });
    sock.on('error', () => {});
    sock.on('close', () => socks.delete(sock));
  });
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () =>
      resolve({
        port: server.address().port,
        conns,
        close: () =>
          new Promise((done) => {
            for (const s of socks) s.destroy();
            server.close(() => done());
          }),
      }),
    );
  });
}

export const delay = (ms) => new Promise((r) => setTimeout(r, ms));

export async function waitUntil(pred, ms = 5000) {
  const end = Date.now() + ms;
  while (!pred() && Date.now() < end) await delay(25);
  return pred();
}

// Plain, unsandboxed spawn for positive controls. Async so that an in-process listener keeps running.
export function runPlain(file, args, { cwd, env, timeoutMs = 60000 }) {
  return new Promise((resolve, reject) => {
    const child = spawn(file, args, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
    const out = { stdout: '', stderr: '' };
    child.stdout.setEncoding('utf8').on('data', (d) => (out.stdout += d));
    child.stderr.setEncoding('utf8').on('data', (d) => (out.stderr += d));
    const timer = setTimeout(() => child.kill('SIGKILL'), timeoutMs);
    child.on('error', (e) => {
      clearTimeout(timer);
      reject(e);
    });
    child.on('close', (exitCode, signal) => {
      clearTimeout(timer);
      resolve({ exitCode, signal, ...out });
    });
  });
}

// The VN_G0C_REPORT line a fixture prints.
export function reportOf(stdout) {
  const line = stdout.split('\n').find((l) => l.startsWith('VN_G0C_REPORT '));
  if (!line) throw new Error(`no VN_G0C_REPORT line in output: ${stdout.slice(0, 400)}`);
  return JSON.parse(line.slice('VN_G0C_REPORT '.length));
}

// Compiles a fixture with the project rustc. Compilation runs unsandboxed: it does not execute the fixture.
export function compileRust(src, out) {
  const env = { ...process.env, RUSTUP_HOME: path.join(ROOT, 'toolchain/rustup'), CARGO_HOME: path.join(ROOT, 'toolchain/cargo') };
  const r = spawnSync(RUSTC, ['--edition=2024', '-D', 'warnings', '-o', out, src], { env, encoding: 'utf8', timeout: 120000 });
  if (r.status !== 0) throw new Error(`rustc failed (${r.status}): ${r.stderr}`);
  return out;
}
