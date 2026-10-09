import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { chromeSkipReason, openPage, serveDirectory } from '../../../tooling/chrome/chrome.mjs';
import { tempDir } from './support.mjs';

const skip = chromeSkipReason();
if (skip) process.stderr.write(`[tooling/chrome] SKIPPED serve.test.mjs: ${skip}\n`);
const skipOpt = skip ? { skip } : {}; // Node 26 treats skip: null as a skip, so only set it when needed

// Hand-assembled empty module: magic "\0asm" then version 1.
const WASM_EMPTY = Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

function request(port, reqPath, method = 'GET') {
  return new Promise((resolve, reject) => {
    const req = http.request({ host: '127.0.0.1', port, path: reqPath, method }, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body: Buffer.concat(chunks) }));
    });
    req.on('error', reject);
    req.end();
  });
}

describe('serveDirectory', () => {
  let base;
  let site;
  let server;
  let page;
  before(async () => {
    if (skip) return;
    base = tempDir('vn-serve-');
    site = path.join(base, 'site');
    fs.mkdirSync(site);
    fs.writeFileSync(path.join(base, 'secret.txt'), 'TOP-SECRET'); // outside the served root
    fs.writeFileSync(
      path.join(site, 'index.html'),
      '<!doctype html><title>served</title><script type="module" src="./app.mjs"></script>',
    );
    fs.writeFileSync(path.join(site, 'app.mjs'), 'window.__mod = { answer: 42 };\n');
    fs.writeFileSync(path.join(site, 'empty.wasm'), WASM_EMPTY);
    fs.writeFileSync(path.join(site, 'empty.bin'), WASM_EMPTY); // same bytes, wrong MIME (negative control)
    fs.writeFileSync(path.join(site, 'dot.png'), Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));
    server = await serveDirectory(site);
    page = await openPage();
  });
  after(async () => {
    if (skip) return;
    await page?.close();
    await server?.close();
    fs.rmSync(base, { recursive: true, force: true });
  });

  test('serves the expected Content-Type for html, module, wasm, png and an unknown extension', skipOpt, async () => {
    const want = {
      '/index.html': 'text/html; charset=utf-8',
      '/app.mjs': 'text/javascript; charset=utf-8',
      '/empty.wasm': 'application/wasm',
      '/dot.png': 'image/png',
      '/empty.bin': 'application/octet-stream',
    };
    for (const [p, type] of Object.entries(want)) {
      const res = await request(server.port, p);
      assert.equal(res.status, 200, p);
      assert.equal(res.headers['content-type'], type, p);
      assert.equal(res.headers['cache-control'], 'no-store', p);
    }
  });

  test('the directory URL serves index.html', skipOpt, async () => {
    const res = await request(server.port, '/');
    assert.equal(res.status, 200);
    assert.match(res.body.toString(), /<title>served<\/title>/);
  });

  test('an ES module referenced by a page executes', skipOpt, async () => {
    await page.goto(server.url);
    assert.deepEqual(await page.evaluate(() => window.__mod), { answer: 42 });
  });

  test('WebAssembly.instantiateStreaming succeeds on the served .wasm', skipOpt, async () => {
    await page.goto(server.url);
    const result = await page.evaluate(async () => {
      const r = await fetch('./empty.wasm');
      const { module, instance } = await WebAssembly.instantiateStreaming(r, {});
      return {
        type: r.headers.get('content-type'),
        isModule: module instanceof WebAssembly.Module,
        exports: Object.keys(instance.exports).length,
      };
    });
    assert.deepEqual(result, { type: 'application/wasm', isModule: true, exports: 0 });
  });

  test('negative control: the same bytes under an octet-stream MIME are refused by instantiateStreaming', skipOpt, async () => {
    await page.goto(server.url);
    const result = await page.evaluate(async () => {
      try {
        await WebAssembly.instantiateStreaming(await fetch('./empty.bin'), {});
        return { ok: true };
      } catch (e) {
        return { ok: false, name: e.name, message: e.message };
      }
    });
    assert.equal(result.ok, false);
    assert.equal(result.name, 'TypeError');
    assert.match(result.message, /MIME/i);
  });

  test('paths that escape the root get 403 and the secret is not served; missing files get 404', skipOpt, async () => {
    const esc = await request(server.port, '/..%2Fsecret.txt');
    assert.equal(esc.status, 403);
    assert.doesNotMatch(esc.body.toString(), /TOP-SECRET/);
    // WHATWG URL parsing folds encoded dot segments, so this lands on /secret.txt inside the root: 404, not served.
    const folded = await request(server.port, '/%2e%2e/secret.txt');
    assert.equal(folded.status, 404);
    assert.doesNotMatch(folded.body.toString(), /TOP-SECRET/);
    assert.equal((await request(server.port, '/missing.html')).status, 404);
  });

  test('only GET and HEAD are served', skipOpt, async () => {
    assert.equal((await request(server.port, '/index.html', 'POST')).status, 405);
  });

  test('extra headers are opt-in (cross-origin isolation)', skipOpt, async () => {
    const isolated = await serveDirectory(site, {
      headers: { 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp' },
    });
    try {
      const res = await request(isolated.port, '/index.html');
      assert.equal(res.headers['cross-origin-opener-policy'], 'same-origin');
      assert.equal(res.headers['cross-origin-embedder-policy'], 'require-corp');
      assert.equal((await request(server.port, '/index.html')).headers['cross-origin-opener-policy'], undefined);
    } finally {
      await isolated.close();
    }
  });

  test('close() stops the server', skipOpt, async () => {
    const short = await serveDirectory(site);
    await short.close();
    await assert.rejects(request(short.port, '/index.html'), { code: 'ECONNREFUSED' });
  });
});
