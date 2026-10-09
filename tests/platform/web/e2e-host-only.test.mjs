// End-to-end (WP-APP1, step b): a web app built by the CLI's packaging pipeline (tooling/cli/build.mjs) and run in
// headless Chrome. The app uses only the browser host (vn-renderer-dom), not the Vue runtime. It proves the generated
// crate, the wasm exports and imports, the glue, the page shell and the manifest before the Vue renderer is wired in.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { buildWebApp, checkWebWasm, toolEnv } from '../../../tooling/cli/build.mjs';
import { chromeSkipReason, serveDirectory, withPage } from '../../../tooling/chrome/chrome.mjs';

const SKIP = chromeSkipReason() ?? false;
// The generated app crate builds vn-renderer-dom outside the framework workspace. Sharing a cargo target directory with the
// workspace-context wasm builds of the other test files in this folder (they run concurrently) leaves two incompatible
// vn_runtime artifacts behind and fails with "multiple different versions of crate `vn_runtime`". This file therefore
// builds its crate in a target directory of its own.
process.env.CARGO_TARGET_DIR = path.join(toolEnv().CARGO_TARGET_DIR, 'host-only-crates');
const OUT = path.join(toolEnv().CARGO_TARGET_DIR, 'e2e');

// The app source is shared with the CLI tests: tests/cli/fixtures/host-only/lib.rs.
const HOST_ONLY_LIB_RS = fs.readFileSync(new URL('../../cli/fixtures/host-only/lib.rs', import.meta.url), 'utf8');

// Negative control: the same app, but it looks for a container the page does not create. vn_start must fail loudly.
const MISSING_CONTAINER_LIB_RS = HOST_ONLY_LIB_RS.replace('vn_config::CONTAINER', '"#missing"');

test('host-only web app: built output is complete and the wasm is a web app', { skip: SKIP }, () => {
  const out = path.join(OUT, 'host-only-files');
  fs.rmSync(out, { recursive: true, force: true });
  const { dir, manifest } = buildWebApp({ out, name: 'host-only', libRs: HOST_ONLY_LIB_RS, title: 'Host only' });
  assert.match(manifest.wasm, /^vn_app_host_only-[0-9a-f]{16}\.wasm$/);
  assert.deepEqual(manifest.wasmImports, 45);
  for (const file of [manifest.wasm, 'vn-host.js', 'index.html', 'manifest.json']) {
    assert.ok(fs.existsSync(path.join(dir, file)), `${file} is written`);
  }
  const check = checkWebWasm(fs.readFileSync(path.join(dir, manifest.wasm)));
  assert.ok(check.exports.includes('vn_start'));
  assert.ok(check.exports.includes('vn_abi_version'));
  const html = fs.readFileSync(path.join(dir, 'index.html'), 'utf8');
  assert.match(html, /<div id="app"><\/div>/);
  console.log(`host-only: wasm ${manifest.bytes} bytes (gzip ${manifest.gzipBytes}), 45 vn_host imports`);
});

test('host-only web app: starts in Chrome and mounts through the browser host', { skip: SKIP }, async () => {
  const out = path.join(OUT, 'host-only');
  fs.rmSync(out, { recursive: true, force: true });
  const { dir, manifest } = buildWebApp({ out, name: 'host-only', libRs: HOST_ONLY_LIB_RS, title: 'Host only' });
  const site = await serveDirectory(dir);
  try {
    await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
      await page.goto(new URL('index.html', site.url).href);
      const ready = await page.evaluate(async () => {
        const host = await window.vnReady;
        const p = document.querySelector('#host-only');
        return {
          abiVersion: host.abiVersion,
          text: p ? p.textContent : null,
          parentId: p && p.parentElement ? p.parentElement.id : null,
          renderedAt: Math.round(performance.now()),
        };
      });
      assert.equal(ready.abiVersion, 1);
      assert.equal(ready.text, 'Hello from the browser host');
      assert.equal(ready.parentId, 'app');
      assert.deepEqual(page.exceptions, []);
      assert.deepEqual(page.consoleLog.filter((e) => e.type === 'error' || e.type === 'warning'), []);
      console.log(`host-only: ${manifest.bytes} bytes wasm; first render ${ready.renderedAt} ms after navigation`);
    });
  } finally {
    await site.close();
  }
});

test('host-only web app: a failing start is reported, not silent (negative control)', { skip: SKIP }, async () => {
  const out = path.join(OUT, 'host-only-missing');
  fs.rmSync(out, { recursive: true, force: true });
  const { dir } = buildWebApp({ out, name: 'host-only-missing', libRs: MISSING_CONTAINER_LIB_RS, title: 'Missing' });
  const site = await serveDirectory(dir);
  try {
    await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
      await page.goto(new URL('index.html', site.url).href);
      const outcome = await page.evaluate(async () => {
        try {
          await window.vnReady;
          return 'started';
        } catch (err) {
          return `failed: ${err.message}`;
        }
      });
      assert.notEqual(outcome, 'started', 'vn_start must not succeed without its container');
      const reported = page.exceptions.length > 0 || page.consoleLog.some((e) => e.type === 'error');
      assert.ok(reported, `the failure is reported (outcome: ${outcome})`);
    });
  } finally {
    await site.close();
  }
});
