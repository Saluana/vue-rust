// Hostile-build isolation tests (WP-G0C, R22.AC1, T22 malicious build-script sentinel).
// For each fixture: the unsandboxed run must perform the attacks (positive control); the sandboxed run must block
// them while workspace I/O still works; a deliberately weakened profile must be caught by the same checks.
// Sandboxed cases are skipped loudly when sandbox-exec is missing. The Rust cases are skipped loudly when the
// project rustc is missing.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { SANDBOX_EXEC, buildProfile, runUntrusted } from '../../../tooling/sandbox/run-untrusted.mjs';
import {
  FIXTURES,
  NO_SANDBOX_SKIP,
  RUST_SKIP,
  compileRust,
  delay,
  fixtureEnv,
  makeRoot,
  reportOf,
  runPlain,
  startListener,
  waitUntil,
} from './support.mjs';

const EPERM = 1;
const WORKSPACE_TEXT = 'WORKSPACE-INPUT\n';
const OK_TEXT = 'workspace-write-ok\n';

// Expectations for a SANDBOXED run. Returns the violated expectations; an empty array means contained.
// Every blocked attack must fail with EPERM (a sandbox denial), not with an unrelated error such as ENOENT.
function containmentViolations(rep, { connections, token, outsideFile, ws, stdout }) {
  const v = [];
  for (const k of ['secretRead', 'outsideWrite', 'networkConnect']) {
    if (rep[k]?.ok) v.push(`${k} succeeded`);
    else if (rep[k]?.errno !== EPERM) v.push(`${k} failed with errno ${rep[k]?.errno} (expected EPERM)`);
  }
  if (connections > 0) v.push(`listener accepted ${connections} connection(s)`);
  if (fs.existsSync(outsideFile)) v.push('outside file was written');
  if (stdout.includes(token)) v.push('sentinel leaked to stdout');
  if (!rep.workspaceRead?.ok || rep.workspaceRead.content !== WORKSPACE_TEXT) v.push('workspace read failed');
  if (!rep.workspaceWrite?.ok || !fs.existsSync(path.join(ws, 'out', 'ok.txt')) || fs.readFileSync(path.join(ws, 'out', 'ok.txt'), 'utf8') !== OK_TEXT) {
    v.push('workspace write failed');
  }
  return v;
}

// Expectations for an UNSANDBOXED run. Returns the attacks that did NOT succeed; empty means a real attack.
function attacksMissing(rep, { connections, token, nonce, outsideFile, ws }) {
  const m = [];
  if (!rep.secretRead?.ok || rep.secretRead.content.trim() !== token) m.push('secretRead');
  if (!rep.outsideWrite?.ok || !fs.existsSync(outsideFile) || fs.readFileSync(outsideFile, 'utf8').trim() !== nonce) m.push('outsideWrite');
  if (!rep.networkConnect?.ok || !connections.some((c) => c.data.includes(nonce))) m.push('networkConnect');
  if (!rep.workspaceRead?.ok) m.push('workspaceRead');
  if (!rep.workspaceWrite?.ok || !fs.existsSync(path.join(ws, 'out', 'ok.txt'))) m.push('workspaceWrite');
  return m;
}

// Plain child process (no sandbox). Used only for the positive control.
function runPlainFixture(t, listener, cmd, args) {
  return runPlain(cmd, args, { cwd: t.ws, env: fixtureEnv(t, listener.port) });
}

// Child process under an explicit profile text, bypassing the runner. Used only for the negative control.
function runUnderProfile(t, listener, profile, cmd, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(SANDBOX_EXEC, ['-p', profile, cmd, ...args], { cwd: t.ws, env: fixtureEnv(t, listener.port), stdio: ['ignore', 'pipe', 'pipe'] });
    const out = { stdout: '', stderr: '' };
    child.stdout.setEncoding('utf8').on('data', (d) => (out.stdout += d));
    child.stderr.setEncoding('utf8').on('data', (d) => (out.stderr += d));
    const timer = setTimeout(() => child.kill('SIGKILL'), 60000);
    child.on('error', (e) => {
      clearTimeout(timer);
      reject(e);
    });
    child.on('close', (exitCode, signal) => {
      clearTimeout(timer);
      resolve({ exitCode, signal, timedOut: false, ...out });
    });
  });
}

// One fixture, three cases. prepare(t) copies or compiles the program into the workspace and returns the command.
function fixtureCases(label, prepare, { skip = false } = {}) {
  describe(label, () => {
    test('positive control: unsandboxed, every attack succeeds (the sentinel and the listener are real)', { skip }, async () => {
      const t = makeRoot();
      try {
        const { cmd, args } = prepare(t);
        const listener = await startListener();
        try {
          const r = await runPlainFixture(t, listener, cmd, args);
          assert.equal(r.exitCode, 0, r.stderr);
          await waitUntil(() => listener.conns.some((c) => c.data.includes(t.nonce)));
          const missing = attacksMissing(reportOf(r.stdout), {
            connections: listener.conns,
            token: t.token,
            nonce: t.nonce,
            outsideFile: path.join(t.outside, 'planted.txt'),
            ws: t.ws,
          });
          assert.deepEqual(missing, [], `attacks that did not succeed unsandboxed: ${missing.join(', ')}`);
        } finally {
          await listener.close();
        }
      } finally {
        t.cleanup();
      }
    });

    test('sandboxed: secret read, outside write and localhost connect are blocked; workspace I/O still works', { skip: skip || NO_SANDBOX_SKIP }, async () => {
      const t = makeRoot();
      try {
        const { cmd, args } = prepare(t);
        const listener = await startListener();
        try {
          const r = await runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd, args, cwd: t.ws, env: fixtureEnv(t, listener.port) });
          assert.equal(r.exitCode, 0, `sandboxed run ended with status ${r.exitCode} signal ${r.signal}: ${r.stderr}`);
          await delay(300);
          const violations = containmentViolations(reportOf(r.stdout), {
            connections: listener.conns.length,
            token: t.token,
            outsideFile: path.join(t.outside, 'planted.txt'),
            ws: t.ws,
            stdout: r.stdout,
          });
          assert.deepEqual(violations, [], `containment violated: ${violations.join('; ')}`);
        } finally {
          await listener.close();
        }
      } finally {
        t.cleanup();
      }
    });

    test('negative control: a deliberately weakened profile is caught by the containment checks', { skip: skip || NO_SANDBOX_SKIP }, async () => {
      const t = makeRoot();
      try {
        const { cmd, args } = prepare(t);
        const real = fs.realpathSync(cmd);
        const listener = await startListener();
        try {
          const weak = `${buildProfile({ workspace: t.ws, scratch: t.scratch, command: real })}\n(allow file-read* file-write* network*)\n`;
          const r = await runUnderProfile(t, listener, weak, real, args);
          assert.equal(r.exitCode, 0, r.stderr);
          await delay(300);
          const violations = containmentViolations(reportOf(r.stdout), {
            connections: listener.conns.length,
            token: t.token,
            outsideFile: path.join(t.outside, 'planted.txt'),
            ws: t.ws,
            stdout: r.stdout,
          });
          assert.ok(violations.includes('secretRead succeeded'), `weakened profile should leak the sentinel: ${violations}`);
          assert.ok(violations.includes('outsideWrite succeeded'), `weakened profile should allow the outside write: ${violations}`);
          assert.ok(violations.some((x) => x.startsWith('listener accepted')), `weakened profile should allow the connect: ${violations}`);
        } finally {
          await listener.close();
        }
      } finally {
        t.cleanup();
      }
    });
  });
}

// npm postinstall-style Node script, copied into the workspace as a checked-out package would have it.
fixtureCases('npm postinstall-style Node script', (t) => {
  fs.copyFileSync(path.join(FIXTURES, 'postinstall.mjs'), path.join(t.ws, 'postinstall.mjs'));
  return { cmd: process.execPath, args: ['postinstall.mjs'] };
});

// Cargo build.rs-style program, compiled into the workspace with the project rustc. Compiling does not run it.
fixtureCases(
  'Cargo build.rs-style Rust program (project rustc, no cargo)',
  (t) => {
    const bin = compileRust(path.join(FIXTURES, 'build-script.rs'), path.join(t.ws, 'build-script'));
    return { cmd: bin, args: [] };
  },
  { skip: RUST_SKIP },
);

// Escape attempts: reach the sentinel from inside the workspace with a symlink and a hard link.
describe('escape attempts through the workspace', () => {
  test('positive control: unsandboxed, the symlink and the hard link both reach the sentinel', async () => {
    const t = makeRoot();
    try {
      fs.copyFileSync(path.join(FIXTURES, 'escape-attempts.mjs'), path.join(t.ws, 'escape-attempts.mjs'));
      const r = await runPlain(process.execPath, ['escape-attempts.mjs'], { cwd: t.ws, env: fixtureEnv(t, 1) });
      assert.equal(r.exitCode, 0, r.stderr);
      const rep = reportOf(r.stdout);
      assert.equal(rep.symlinkRead?.value, `${t.token}\n`, JSON.stringify(rep));
      assert.equal(rep.hardlinkRead?.value, `${t.token}\n`, JSON.stringify(rep));
    } finally {
      t.cleanup();
    }
  });

  test('sandboxed: a symlink or hard link inside the workspace cannot be read through', { skip: NO_SANDBOX_SKIP }, async () => {
    const t = makeRoot();
    try {
      fs.copyFileSync(path.join(FIXTURES, 'escape-attempts.mjs'), path.join(t.ws, 'escape-attempts.mjs'));
      const r = await runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: process.execPath, args: ['escape-attempts.mjs'], cwd: t.ws, env: fixtureEnv(t, 1) });
      assert.equal(r.exitCode, 0, r.stderr);
      const rep = reportOf(r.stdout);
      assert.equal(rep.symlinkRead?.ok, false, JSON.stringify(rep));
      assert.equal(rep.symlinkRead?.errno, EPERM, JSON.stringify(rep));
      assert.equal(rep.hardlinkCreate?.ok, false, JSON.stringify(rep));
      assert.equal(rep.hardlinkCreate?.errno, EPERM, JSON.stringify(rep));
      assert.ok(!r.stdout.includes(t.token), 'sentinel must not leak');
    } finally {
      t.cleanup();
    }
  });
});
