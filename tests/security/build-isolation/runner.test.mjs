// Tests for tooling/sandbox/run-untrusted.mjs (WP-G0C). Cases that need a working sandbox-exec are skipped loudly
// when it is missing; cases that check refusals run everywhere.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  EXIT,
  SandboxError,
  assertSandboxEffective,
  buildProfile,
  plan,
  runUntrusted,
} from '../../../tooling/sandbox/run-untrusted.mjs';
import { NO_SANDBOX_SKIP, RUNNER, delay, makeRoot, runPlain, waitUntil } from './support.mjs';

const NODE = process.execPath;
const PLATFORM_SKIP = process.platform === 'darwin' ? false : `LOUD SKIP: sandbox-exec exists only on macOS (platform ${process.platform}).`;
// Node program that writes a marker file; used to prove a command did or did not run.
const markerScript = (file) => `require('fs').writeFileSync(${JSON.stringify(file)}, 'ran')`;

describe('profile generation', () => {
  test('deny-default, no network, and writes limited to workspace and scratch', () => {
    const t = makeRoot();
    try {
      const profile = buildProfile({ workspace: t.ws, scratch: t.scratch, command: NODE });
      assert.match(profile, /^\(version 1\)\n\(deny default\)/);
      assert.match(profile, /\(deny network\*\)/);
      assert.ok(!profile.includes(`(subpath "${t.home}")`), 'fake home must not be granted');
      const realHome = fs.realpathSync(os.homedir());
      assert.ok(!profile.includes(`(subpath "${realHome}")`), 'real home must not be granted');
      const allowedWrite = new Set([fs.realpathSync(t.ws), fs.realpathSync(t.scratch), '/dev/null']);
      const writeRules = profile.split('\n').filter((l) => /^\(allow .*file-write/.test(l));
      assert.ok(writeRules.length >= 2, 'expected write rules');
      for (const rule of writeRules) {
        for (const [, p] of rule.matchAll(/"((?:[^"\\]|\\.)*)"/g)) {
          assert.ok(allowedWrite.has(p.replace(/\\\\/g, '\\').replace(/\\"/g, '"')), `unexpected write grant: ${p}`);
        }
      }
    } finally {
      t.cleanup();
    }
  });

  test('SBPL escaping: quotes and backslashes are escaped and sandbox-exec loads the profile', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, async () => {
    const t = makeRoot();
    try {
      const weird = path.join(t.root, 'we"ird\\name');
      fs.mkdirSync(weird);
      const profile = buildProfile({ workspace: weird, scratch: t.scratch, command: '/usr/bin/true' });
      const escaped = fs.realpathSync(weird).replace(/\\/g, '\\\\').replace(/"/g, '\\"');
      assert.ok(profile.includes(`(subpath "${escaped}")`));
      const env = { PATH: '/usr/bin:/bin' };
      const ok = await runPlain('/usr/bin/sandbox-exec', ['-p', profile, '/usr/bin/true'], { cwd: t.ws, env });
      assert.equal(ok.exitCode, 0, ok.stderr);
      // Negative control: the same profile with the escaping removed must be rejected by sandbox-exec.
      const broken = profile.replace(escaped, escaped.replace(/\\"/g, '"'));
      assert.notEqual(broken, profile);
      const bad = await runPlain('/usr/bin/sandbox-exec', ['-p', broken, '/usr/bin/true'], { cwd: t.ws, env });
      assert.notEqual(bad.exitCode, 0, 'unescaped profile should not load');
    } finally {
      t.cleanup();
    }
  });

  test('control characters in paths are rejected, so a path cannot inject profile rules', () => {
    const t = makeRoot();
    try {
      const evil = path.join(t.root, 'a\nb');
      fs.mkdirSync(evil);
      assert.throws(() => buildProfile({ workspace: evil, scratch: t.scratch, command: NODE }), /control characters/);
    } finally {
      t.cleanup();
    }
  });

  test('refuses workspaces that are /, the home or temp directory, a parent of them, or contain system trees', () => {
    for (const bad of ['/', os.homedir(), os.tmpdir(), path.dirname(os.homedir()), '/tmp', '/usr']) {
      assert.throws(() => plan({ workspace: bad, cmd: NODE }), (e) => e instanceof SandboxError && e.exitCode === EXIT.USAGE, bad);
    }
  });

  test('refuses a workspace that overlaps a credential directory', (ctx) => {
    const ssh = path.join(os.homedir(), '.ssh');
    if (!fs.existsSync(ssh)) {
      ctx.diagnostic('~/.ssh is absent on this machine: the credential-overlap case was not exercised');
      return;
    }
    assert.throws(() => plan({ workspace: ssh, cmd: NODE }), (e) => e instanceof SandboxError && e.exitCode === EXIT.USAGE);
  });

  test('relative commands resolve against the working directory; unknown commands are refused', () => {
    const t = makeRoot();
    try {
      const tool = path.join(t.ws, 'tool.sh');
      fs.writeFileSync(tool, '#!/bin/sh\n', { mode: 0o755 });
      const p = plan({ workspace: t.ws, scratch: t.scratch, cmd: './tool.sh' });
      assert.equal(p.command, fs.realpathSync(tool));
      assert.throws(() => plan({ workspace: t.ws, scratch: t.scratch, cmd: 'no-such-command-g0c' }), /not found/);
    } finally {
      t.cleanup();
    }
  });
});

describe('refusals never run the command', () => {
  test('runUntrusted rejects with exit 97 when sandbox-exec is missing, and the command does not run', async () => {
    const t = makeRoot();
    try {
      const marker = path.join(t.ws, 'ran.txt');
      await assert.rejects(
        runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: NODE, args: ['-e', markerScript(marker)], sandboxExec: '/nonexistent/sandbox-exec' }),
        (e) => e instanceof SandboxError && e.code === 'UNAVAILABLE' && e.exitCode === EXIT.UNAVAILABLE,
      );
      assert.equal(fs.existsSync(marker), false);
    } finally {
      t.cleanup();
    }
  });

  test('CLI exits 97 for a missing sandbox-exec and for a binary not named sandbox-exec', async () => {
    const t = makeRoot();
    try {
      const marker = path.join(t.ws, 'ran.txt');
      for (const sandboxExec of ['/nonexistent/sandbox-exec', '/usr/bin/true']) {
        const r = await runPlain(NODE, [RUNNER, '--sandbox-exec', sandboxExec, '--workspace', t.ws, '--cmd', NODE, '-e', markerScript(marker)], {
          cwd: t.ws,
          env: { PATH: '/usr/bin:/bin' },
        });
        assert.equal(r.exitCode, 97, `${sandboxExec}: ${r.stderr}`);
        assert.match(r.stderr, /unavailable/);
      }
      assert.equal(fs.existsSync(marker), false);
    } finally {
      t.cleanup();
    }
  });

  test('CLI usage errors exit 2', async () => {
    const r1 = await runPlain(NODE, [RUNNER, '--cmd', NODE, '-e', '0'], { env: { PATH: '/usr/bin:/bin' } });
    assert.equal(r1.exitCode, 2);
    assert.match(r1.stderr, /--workspace DIR is required/);
    const r2 = await runPlain(NODE, [RUNNER, '--workspace', os.tmpdir(), '--bogus'], { env: { PATH: '/usr/bin:/bin' } });
    assert.equal(r2.exitCode, 2);
  });

  test('self-test refuses a no-op sandbox-exec (exit 98) and the command does not run', { skip: PLATFORM_SKIP }, async () => {
    const t = makeRoot();
    try {
      const fake = path.join(t.root, 'fake', 'sandbox-exec');
      fs.mkdirSync(path.dirname(fake));
      // A "sandbox" that ignores the profile: it drops -p and the profile, then runs the command.
      fs.writeFileSync(fake, '#!/bin/sh\nshift 2\nexec "$@"\n', { mode: 0o755 });
      const marker = path.join(t.ws, 'ran.txt');
      await assert.rejects(
        runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: NODE, args: ['-e', markerScript(marker)], sandboxExec: fake }),
        (e) => e instanceof SandboxError && e.code === 'INEFFECTIVE' && e.exitCode === EXIT.INEFFECTIVE,
      );
      assert.equal(fs.existsSync(marker), false);
    } finally {
      t.cleanup();
    }
  });
});

describe('sandboxed execution (requires sandbox-exec)', () => {
  test('self-test passes on this machine', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, () => {
    const t = makeRoot();
    try {
      assertSandboxEffective({ workspace: t.ws, scratch: t.scratch });
    } finally {
      t.cleanup();
    }
  });

  test('the child inherits no caller environment; HOME and TMPDIR point into the workspace', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, async () => {
    const t = makeRoot();
    process.env.VN_G0C_CALLER_SECRET = 'caller-only';
    try {
      const r = await runUntrusted({
        workspace: t.ws,
        scratch: t.scratch,
        cmd: NODE,
        args: ['-e', "console.log(JSON.stringify({s: process.env.VN_G0C_CALLER_SECRET ?? null, home: process.env.HOME, tmp: process.env.TMPDIR}))"],
      });
      assert.equal(r.exitCode, 0, r.stderr);
      const seen = JSON.parse(r.stdout);
      assert.equal(seen.s, null);
      assert.equal(seen.home, fs.realpathSync(t.ws));
      assert.equal(seen.tmp, fs.realpathSync(t.scratch));
    } finally {
      delete process.env.VN_G0C_CALLER_SECRET;
      t.cleanup();
    }
  });

  test('--read trees are readable but not writable; without them the same path is denied', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, async () => {
    const t = makeRoot();
    try {
      const cache = path.join(t.root, 'cache');
      fs.mkdirSync(cache);
      fs.writeFileSync(path.join(cache, 'pkg.txt'), 'cached-data\n');
      const script = [
        "const fs = require('fs');",
        `const f = ${JSON.stringify(path.join(cache, 'pkg.txt'))};`,
        `const g = ${JSON.stringify(path.join(cache, 'x.txt'))};`,
        'const out = {};',
        "try { out.read = fs.readFileSync(f, 'utf8'); } catch (e) { out.read = e.code; }",
        "try { fs.writeFileSync(g, 'w'); out.write = 'ok'; } catch (e) { out.write = e.code; }",
        'console.log(JSON.stringify(out));',
      ].join('\n');
      const without = await runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: NODE, args: ['-e', script] });
      assert.deepEqual(JSON.parse(without.stdout), { read: 'EPERM', write: 'EPERM' });
      const withRead = await runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: NODE, args: ['-e', script], readPaths: [cache] });
      assert.deepEqual(JSON.parse(withRead.stdout), { read: 'cached-data\n', write: 'EPERM' });
    } finally {
      t.cleanup();
    }
  });

  test('a caller-named TZ reaches the child (Intl reports UTC otherwise, because /etc/localtime is denied)', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, async () => {
    const t = makeRoot();
    try {
      const args = ['-e', 'console.log(Intl.DateTimeFormat().resolvedOptions().timeZone)'];
      const plain = await runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: NODE, args });
      assert.equal(plain.stdout.trim(), 'UTC');
      const named = await runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: NODE, args, env: { TZ: 'Asia/Tokyo' } });
      assert.equal(named.stdout.trim(), 'Asia/Tokyo');
    } finally {
      t.cleanup();
    }
  });

  test('timeouts kill the sandboxed child', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, async () => {
    const t = makeRoot();
    try {
      const r = await runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: NODE, args: ['-e', 'setTimeout(() => {}, 60000)'], timeoutMs: 1500 });
      assert.equal(r.timedOut, true);
      assert.equal(r.signal, 'SIGKILL');
    } finally {
      t.cleanup();
    }
  });

  // The command starts a background process, writes its pid to the workspace, then waits.
  const grandchildScript = (pidFile) => ['-c', 'sleep 30 & echo $! > "$1"; wait', 'sh', pidFile];
  const alive = (pid) => {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  };

  test('timeouts kill the whole process group, not only the direct child', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, async () => {
    const t = makeRoot();
    try {
      const pidFile = path.join(t.ws, 'grandchild.pid');
      const t0 = Date.now();
      const r = await runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: '/bin/sh', args: grandchildScript(pidFile), timeoutMs: 1500 });
      const elapsed = Date.now() - t0;
      assert.equal(r.timedOut, true);
      // Without the group kill the runner waits for the background sleep (30 s) to finish on its own.
      assert.ok(elapsed < 10000, `runner returned after ${elapsed} ms; it waited for the background process`);
      const pid = Number(fs.readFileSync(pidFile, 'utf8'));
      await delay(200);
      assert.equal(alive(pid), false, `background process ${pid} survived the timeout`);
    } finally {
      t.cleanup();
    }
  });

  test('a process that leaves the group cannot hang the runner; output is cut off after a grace period', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, async () => {
    const t = makeRoot();
    const pidFile = path.join(t.ws, 'escaped.pid');
    try {
      // setsid-like escape: a detached child that inherits the output pipes and outlives the command.
      const code = [
        "const { spawn } = require('child_process');",
        "const c = spawn('/bin/sleep', ['30'], { detached: true, stdio: 'inherit' });",
        `require('fs').writeFileSync(${JSON.stringify(pidFile)}, String(c.pid));`,
        'c.unref();',
      ].join('\n');
      const t0 = Date.now();
      const r = await runUntrusted({ workspace: t.ws, scratch: t.scratch, cmd: NODE, args: ['-e', code], timeoutMs: 10000 });
      const elapsed = Date.now() - t0;
      assert.equal(r.exitCode, 0, r.stderr);
      assert.equal(r.timedOut, false);
      assert.ok(elapsed < 8000, `runner took ${elapsed} ms; it should stop waiting after the grace period`);
    } finally {
      if (fs.existsSync(pidFile)) {
        try {
          process.kill(Number(fs.readFileSync(pidFile, 'utf8')), 'SIGKILL');
        } catch {
          // Already gone.
        }
      }
      t.cleanup();
    }
  });

  test('CLI: SIGTERM to the runner kills the command group and the runner then exits by that signal', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, async () => {
    const t = makeRoot();
    const pidFile = path.join(t.ws, 'grandchild.pid');
    const child = spawn(NODE, [RUNNER, '--workspace', t.ws, '--scratch', t.scratch, '--cmd', '/bin/sh', ...grandchildScript(pidFile)], {
      stdio: ['ignore', 'ignore', 'ignore'],
      env: { PATH: '/usr/bin:/bin' },
    });
    try {
      assert.ok(await waitUntil(() => fs.existsSync(pidFile), 10000), 'command did not start');
      child.kill('SIGTERM');
      const exit = await new Promise((resolve) => child.on('close', (code, signal) => resolve({ code, signal })));
      assert.deepEqual(exit, { code: null, signal: 'SIGTERM' });
      const pid = Number(fs.readFileSync(pidFile, 'utf8'));
      await delay(200);
      assert.equal(alive(pid), false, `background process ${pid} survived SIGTERM`);
    } finally {
      if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
      t.cleanup();
    }
  });

  test('CLI forwards the child status and output even when stdout is a file outside the workspace', { skip: NO_SANDBOX_SKIP || PLATFORM_SKIP }, async () => {
    // Regression: inheriting a regular file outside the workspace made sandboxed Node abort at startup.
    const t = makeRoot();
    try {
      const out = path.join(t.root, 'cli-stdout.txt');
      const fd = fs.openSync(out, 'w');
      const code = await new Promise((resolve, reject) => {
        const child = spawn(NODE, [RUNNER, '--workspace', t.ws, '--scratch', t.scratch, '--cmd', NODE, '-e', "console.log('fwd-ok'); process.exit(7)"], {
          stdio: ['ignore', fd, fd],
          env: { PATH: '/usr/bin:/bin' },
        });
        child.on('error', reject);
        child.on('close', resolve);
      });
      fs.closeSync(fd);
      assert.equal(code, 7, fs.readFileSync(out, 'utf8'));
      assert.match(fs.readFileSync(out, 'utf8'), /fwd-ok/);
    } finally {
      t.cleanup();
    }
  });
});
