import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { once } from 'node:events';
import { chromeSkipReason, launchChrome, openPage, withPage } from '../../../tooling/chrome/chrome.mjs';
import { eventually, firstLine, isAlive, pgrepDir, tempDir } from './support.mjs';

const CHROME_MODULE = new URL('../../../tooling/chrome/chrome.mjs', import.meta.url).href;
const skip = chromeSkipReason();
const skipOpt = skip ? { skip } : {}; // Node 26 treats skip: null as a skip, so only set it when needed
if (skip) process.stderr.write(`[tooling/chrome] SKIPPED launch.test.mjs: ${skip}\n`);

// Child process: launches Chrome, reports its pid and profile dir on stdout, then runs `ending`.
const holderScript = (ending) =>
  [
    'const { launchChrome } = await import(process.argv[1]);',
    'const b = await launchChrome();',
    'process.stdout.write(JSON.stringify({ pid: b.pid, dir: b.userDataDir }) + "\\n");',
    ending,
  ].join('\n');

function withEnv(name, value, fn) {
  const saved = process.env[name];
  process.env[name] = value;
  return fn().finally(() => {
    if (saved === undefined) delete process.env[name];
    else process.env[name] = saved;
  });
}

function fakeChrome(body) {
  const dir = tempDir('vn-fake-');
  const script = path.join(dir, 'fake-chrome.sh');
  fs.writeFileSync(script, `#!/bin/sh\n${body}`);
  fs.chmodSync(script, 0o755);
  return { dir, script, cleanup: () => fs.rmSync(dir, { recursive: true, force: true }) };
}

describe('launchChrome and close', () => {
  test('launch exposes the DevTools endpoint; close is idempotent and leaves nothing behind', skipOpt, async () => {
    const browser = await launchChrome();
    const dir = browser.userDataDir;
    try {
      assert.match(browser.wsUrl, /^ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[0-9A-Za-z-]+$/);
      assert.equal(browser.port, Number(new URL(browser.wsUrl).port));
      assert.ok(path.basename(dir).startsWith('vn-chrome-'));
      assert.equal(fs.statSync(dir).isDirectory(), true);
      assert.ok((await pgrepDir(dir)).includes(browser.pid), 'the browser process carries its profile dir');
    } finally {
      await browser.close();
      await browser.close();
    }
    assert.deepEqual(await pgrepDir(dir), []);
    assert.equal(isAlive(browser.pid), false);
    assert.equal(fs.existsSync(dir), false);
  });

  test('close kills the browser and every helper process it started', skipOpt, async () => {
    const page = await openPage();
    const dir = page.userDataDir;
    const before = await pgrepDir(dir);
    assert.ok(before.includes(page.pid));
    assert.ok(before.length >= 2, `expected helper processes besides the browser, saw ${before.length}`);
    await page.close();
    for (const pid of before) assert.equal(isAlive(pid), false, `pid ${pid} survived close()`);
    assert.deepEqual(await pgrepDir(dir), []);
    assert.equal(fs.existsSync(dir), false);
  });

  test('withPage closes Chrome when the callback throws', skipOpt, async () => {
    let dir;
    await assert.rejects(
      withPage(async (page) => {
        dir = page.userDataDir;
        throw new Error('deliberate');
      }),
      { message: 'deliberate' },
    );
    assert.deepEqual(await pgrepDir(dir), []);
    assert.equal(fs.existsSync(dir), false);
  });

  test('SIGTERM to the process that owns Chrome kills Chrome and removes its profile dir', skipOpt, async () => {
    const child = spawn(process.execPath, ['--input-type=module', '-e', holderScript('setInterval(() => {}, 1000);'), CHROME_MODULE], {
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stderr = '';
    child.stderr.on('data', (d) => (stderr += d));
    const info = JSON.parse(await firstLine(child.stdout));
    child.kill('SIGTERM');
    const [code, signal] = await once(child, 'exit');
    assert.ok(code === 143 || signal === 'SIGTERM', `unexpected exit ${code} ${signal}: ${stderr}`);
    await eventually(async () => !isAlive(info.pid) && (await pgrepDir(info.dir)).length === 0);
    assert.equal(fs.existsSync(info.dir), false);
  });

  test('process.exit without close() still kills Chrome and removes its profile dir', skipOpt, async () => {
    const child = spawn(process.execPath, ['--input-type=module', '-e', holderScript('process.exit(0);'), CHROME_MODULE], {
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const info = JSON.parse(await firstLine(child.stdout));
    const [code] = await once(child, 'exit');
    assert.equal(code, 0);
    await eventually(async () => !isAlive(info.pid) && (await pgrepDir(info.dir)).length === 0);
    assert.equal(fs.existsSync(info.dir), false);
  });

  test('managed flags and non-string args are rejected before Chrome starts', skipOpt, async () => {
    await assert.rejects(launchChrome({ args: ['--user-data-dir=/tmp/elsewhere'] }), /set by the runner/);
    await assert.rejects(launchChrome({ args: ['--remote-debugging-port=9222'] }), /set by the runner/);
    await assert.rejects(launchChrome({ args: ['--headless=old'] }), /set by the runner/);
    await assert.rejects(launchChrome({ args: [42] }), TypeError);
  });

  test('a CHROME_PATH that is not executable is an error, not a silent fallback', skipOpt, async () => {
    await withEnv('CHROME_PATH', '/nonexistent/chrome-for-test', () =>
      assert.rejects(launchChrome(), /CHROME_PATH=\/nonexistent\/chrome-for-test is not an executable file/),
    );
  });

  test('a binary that never prints DevTools is killed and its profile dir removed', skipOpt, async () => {
    const fake = fakeChrome('echo "$$" > "$(dirname "$0")/pid.txt"\necho "$@" > "$(dirname "$0")/args.txt"\nexec sleep 30\n');
    try {
      await withEnv('CHROME_PATH', fake.script, async () => {
        await assert.rejects(launchChrome({ timeout: 500 }), /no "DevTools listening on" line within 500 ms/);
      });
      const pid = Number(fs.readFileSync(path.join(fake.dir, 'pid.txt'), 'utf8'));
      const profile = /--user-data-dir=(\S+)/.exec(fs.readFileSync(path.join(fake.dir, 'args.txt'), 'utf8'))[1];
      assert.equal(isAlive(pid), false, 'the fake browser must be dead after the failed launch');
      assert.equal(fs.existsSync(profile), false, 'its profile dir must be removed');
    } finally {
      fake.cleanup();
    }
  });

  test('a binary that exits before DevTools is reported with its status and output', skipOpt, async () => {
    const fake = fakeChrome('echo "boom from fake chrome" 1>&2\nexit 3\n');
    try {
      await withEnv('CHROME_PATH', fake.script, () =>
        assert.rejects(launchChrome({ timeout: 5000 }), (err) => {
          assert.match(err.message, /exited before DevTools was ready \(code 3, signal null\)/);
          assert.match(err.message, /boom from fake chrome/);
          return true;
        }),
      );
    } finally {
      fake.cleanup();
    }
  });
});
