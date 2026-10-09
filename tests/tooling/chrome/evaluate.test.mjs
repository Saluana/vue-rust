import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import { chromeSkipReason, openPage, withPage } from '../../../tooling/chrome/chrome.mjs';
import { eventually, isAlive, pgrepDir, tempDir } from './support.mjs';

const skip = chromeSkipReason();
if (skip) process.stderr.write(`[tooling/chrome] SKIPPED evaluate.test.mjs: ${skip}\n`);
const skipOpt = skip ? { skip } : {}; // Node 26 treats skip: null as a skip, so only set it when needed

describe('page.evaluate', () => {
  let page;
  before(async () => {
    if (skip) return;
    page = await openPage();
    await page.setContent('<!doctype html><title>t</title><body></body>');
  });
  after(async () => {
    if (skip) return;
    await page?.close();
  });

  test('numbers, strings, booleans and null round-trip', skipOpt, async () => {
    assert.equal(await page.evaluate(() => 42), 42);
    assert.equal(await page.evaluate(() => -1.25), -1.25);
    assert.equal(await page.evaluate(() => 'héllo "quoted"\n'), 'héllo "quoted"\n');
    assert.equal(await page.evaluate(() => true), true);
    assert.equal(await page.evaluate(() => null), null);
  });

  test('plain objects and arrays round-trip', skipOpt, async () => {
    assert.deepEqual(await page.evaluate(() => ({ a: 1, b: 'x', c: { d: [true, null, 2.5] } })), {
      a: 1,
      b: 'x',
      c: { d: [true, null, 2.5] },
    });
    assert.deepEqual(await page.evaluate(() => [1, 'a', null, [2, { z: 0 }]]), [1, 'a', null, [2, { z: 0 }]]);
  });

  test('undefined is kept at top level and inside containers', skipOpt, async () => {
    assert.equal(await page.evaluate(() => undefined), undefined);
    const got = await page.evaluate(() => ({ a: undefined, b: [undefined, 1] }));
    assert.deepStrictEqual(got, { a: undefined, b: [undefined, 1] });
    assert.ok('a' in got);
  });

  test('NaN, Infinity, -Infinity, -0 and BigInt come back as strings (documented convention)', skipOpt, async () => {
    assert.equal(await page.evaluate(() => NaN), 'NaN');
    assert.deepEqual(await page.evaluate(() => [Infinity, -Infinity, -0, { n: NaN }]), ['Infinity', '-Infinity', '-0', { n: 'NaN' }]);
    assert.equal(await page.evaluate(() => 2n ** 70n), '1180591620717411303424');
  });

  test('arguments reach the page as real values: undefined, NaN, -0, BigInt, nested data', skipOpt, async () => {
    assert.equal(await page.evaluate((a, b) => a + b, 2, 3), 5);
    assert.equal(await page.evaluate((x) => x === undefined, undefined), true);
    assert.equal(await page.evaluate((x) => Number.isNaN(x), NaN), true);
    assert.equal(await page.evaluate((x) => Object.is(x, -0), -0), true);
    assert.equal(await page.evaluate((o) => o.list.map((v) => v * 2).join(','), { list: [1, 2, 3] }), '2,4,6');
    assert.equal(await page.evaluate((o) => 'a' in o && o.a === undefined, { a: undefined }), true);
    assert.equal(await page.evaluate((x) => typeof x, 10n), 'bigint');
  });

  test('async functions and promises are awaited', skipOpt, async () => {
    assert.equal(
      await page.evaluate(async (ms) => {
        await new Promise((r) => setTimeout(r, ms));
        return 'done';
      }, 10),
      'done',
    );
    assert.deepEqual(await page.evaluate(() => Promise.resolve({ ok: 1 })), { ok: 1 });
  });

  test('expression strings run in the page; extra arguments with a string are rejected', skipOpt, async () => {
    assert.equal(await page.evaluate('1 + 2'), 3);
    assert.equal(await page.evaluate('document.title'), 't');
    assert.equal(await page.evaluate('(async () => { await null; return "later"; })()'), 'later');
    await assert.rejects(page.evaluate('1', 2), /extra arguments are only supported/);
  });

  test('page exceptions reject with the original name and message', skipOpt, async () => {
    await assert.rejects(
      page.evaluate(() => {
        throw new Error('boom-42');
      }),
      (e) => e instanceof Error && e.name === 'Error' && e.message === 'boom-42',
    );
    await assert.rejects(page.evaluate(() => null.x), (e) => e instanceof TypeError && /reading 'x'/.test(e.message));
    await assert.rejects(
      page.evaluate(() => {
        throw 'plain';
      }),
      { message: 'plain' },
    );
    await assert.rejects(page.evaluate('throw new RangeError("range-7")'), (e) => e instanceof RangeError && e.message === 'range-7');
    await assert.rejects(page.evaluate('1 +'), (e) => e instanceof SyntaxError);
  });

  test('negative controls: values that cannot be sent back are rejected, not approximated', skipOpt, async () => {
    const cases = [
      [
        () => Symbol('s'),
        /cannot return a symbol value/,
      ],
      [() => () => 1, /cannot return a function value/],
      [() => new Date(0), /non-plain object \(Date\)/],
      [() => document.body, /non-plain object \(HTMLBodyElement\)/],
      [
        () => {
          const o = {};
          o.self = o;
          return o;
        },
        /cyclic/,
      ],
      [() => ({ $t: 'x' }), /reserved/],
      [Math.max, /native functions/],
    ];
    for (const [fn, re] of cases) {
      await assert.rejects(page.evaluate(fn), (e) => e instanceof TypeError && re.test(e.message), String(re));
    }
  });

  test('console calls are captured in order with their CDP type', skipOpt, async () => {
    const start = page.consoleLog.length;
    await page.evaluate(() => {
      console.log('hello', 1, true);
      console.error('bad');
      console.warn('careful');
    });
    assert.deepEqual(page.consoleLog.slice(start), [
      { type: 'log', text: 'hello 1 true' },
      { type: 'error', text: 'bad' },
      { type: 'warning', text: 'careful' },
    ]);
  });

  test('uncaught page exceptions are captured', skipOpt, async () => {
    await page.evaluate(() => {
      setTimeout(() => {
        throw new Error('late-boom');
      }, 0);
    });
    await eventually(() => page.exceptions.some((e) => /late-boom/.test(e.message)));
  });

  test('goto accepts a local file path', skipOpt, async () => {
    const dir = tempDir('vn-goto-');
    try {
      const file = path.join(dir, 'page.html');
      await fsp.writeFile(file, '<!doctype html><title>from-file</title><p>x</p>');
      await page.goto(file);
      assert.equal(await page.evaluate(() => document.title), 'from-file');
      assert.equal(await page.evaluate(() => location.protocol), 'file:');
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  test('viewport() sets size and DPR; screenshot writes a PNG of that size', skipOpt, async () => {
    const dir = tempDir('vn-shot-');
    try {
      await page.viewport(300, 200, 2);
      assert.deepEqual(await page.evaluate(() => [innerWidth, innerHeight, devicePixelRatio]), [300, 200, 2]);
      await page.viewport(800, 600, 1);
      await page.setContent('<!doctype html><body style="margin:0;background:#ff0000"></body>');
      const out = await page.screenshot(path.join(dir, 'nested', 'shot.png'));
      const png = await fsp.readFile(out);
      assert.deepEqual([...png.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
      assert.equal(png.readUInt32BE(16), 800);
      assert.equal(png.readUInt32BE(20), 600);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe('timeouts and cleanup', () => {
  test('a hanging evaluate rejects after its timeout and Chrome still closes', skipOpt, async () => {
    let pid;
    let dir;
    const started = Date.now();
    await assert.rejects(
      withPage({ timeout: 400 }, async (p) => {
        pid = p.pid;
        dir = p.userDataDir;
        await p.evaluate(() => new Promise(() => {}));
      }),
      { message: 'evaluate timed out after 400 ms' },
    );
    assert.ok(Date.now() - started < 10000, 'the timeout should fire well before the test budget');
    assert.equal(isAlive(pid), false);
    assert.deepEqual(await pgrepDir(dir), []);
    assert.equal(fs.existsSync(dir), false);
  });

  test('a busy-looping page times out and Chrome is still killed', skipOpt, async () => {
    let pid;
    let dir;
    await assert.rejects(
      withPage({ timeout: 400 }, async (p) => {
        pid = p.pid;
        dir = p.userDataDir;
        await p.evaluate(() => {
          for (;;) {
            // never returns: the renderer stays busy
          }
        });
      }),
      { message: 'evaluate timed out after 400 ms' },
    );
    assert.equal(isAlive(pid), false);
    assert.deepEqual(await pgrepDir(dir), []);
    assert.equal(fs.existsSync(dir), false);
  });
});
