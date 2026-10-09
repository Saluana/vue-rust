// Shared helpers for the browser-host tests (WP-W1a). Each test runs in headless Chrome (tooling/chrome) against the
// self-test build of the site (tooling/web/build-wasm.mjs). Operations go through the real wasm imports and the real glue;
// expectations are hand-written and checked against the DOM the page holds afterwards.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromeSkipReason, openPage, serveDirectory } from '../../../tooling/chrome/chrome.mjs';
import { buildWebDist, defaultOutDir } from '../../../tooling/web/build-wasm.mjs';

export const ROOT = fileURLToPath(new URL('../../../', import.meta.url));
export const skip = chromeSkipReason();
if (skip) process.stderr.write(`[platform/web] SKIPPED: ${skip}\n`);
export const skipOpt = skip ? { skip } : {};

const SOURCES = [path.join(ROOT, 'crates', 'renderer-dom', 'src'), path.join(ROOT, 'hosts', 'web')];

function newestMtime(dir) {
  let newest = 0;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    const t = entry.isDirectory() ? newestMtime(full) : fs.statSync(full).mtimeMs;
    newest = Math.max(newest, t);
  }
  return newest;
}

const sleep = (ms) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);

// Builds the site when it is missing or older than its sources. A lock directory keeps parallel test files from building twice.
export function ensureSite() {
  const out = process.env.VN_WEB_DIST ?? defaultOutDir();
  const manifest = path.join(out, 'selftest', 'manifest.json');
  const stale = () => !fs.existsSync(manifest) || newestMtime(SOURCES[0]) > fs.statSync(manifest).mtimeMs
    || newestMtime(SOURCES[1]) > fs.statSync(manifest).mtimeMs;
  if (!stale()) return path.join(out, 'selftest');
  fs.mkdirSync(out, { recursive: true });
  const lock = path.join(out, '.build-lock');
  const deadline = Date.now() + 15 * 60_000;
  for (;;) {
    try {
      fs.mkdirSync(lock);
      break;
    } catch (err) {
      if (err.code !== 'EEXIST') throw err;
      if (!stale()) return path.join(out, 'selftest');
      if (Date.now() > deadline) throw new Error(`timed out waiting for ${lock}`);
      sleep(250);
    }
  }
  try {
    if (stale()) buildWebDist({ outDir: out });
  } finally {
    fs.rmdirSync(lock);
  }
  return path.join(out, 'selftest');
}

// The served site: VN_WEB_SITE may name another selftest directory (the negative control uses a broken copy).
export async function startSite() {
  const dir = process.env.VN_WEB_SITE ? path.resolve(process.env.VN_WEB_SITE) : ensureSite();
  return serveDirectory(dir);
}

// An async expression (Page.evaluate runs strings as expressions and awaits the promise).
const READY = `(async () => {
  for (let i = 0; i < 500 && !window.vnReady; i++) await new Promise((r) => setTimeout(r, 10));
  await window.vnReady;
  return true;
})()`;

// Opens a fresh page and waits for the host. Use reloadHost to start over in the same browser.
export async function openHostPage(site) {
  const page = await openPage({ viewport: { width: 800, height: 600, dpr: 1 } });
  try {
    await reloadHost(page, site);
  } catch (err) {
    await page.close();
    throw err;
  }
  return page;
}

export async function reloadHost(page, site) {
  await page.goto(new URL('index.html', site.url).href);
  await page.evaluate(READY);
}

// Runs a request of operations through the self-test export and returns one result per operation.
// A result is { ok: value } or { err: { kind, msg } }; { harness: message } means the test asked for something invalid.
export function runOps(page, ops, hostName = 'vnHost') {
  return page.evaluate(async ([request, name]) => {
    const ex = window[name].exports;
    const bytes = new TextEncoder().encode(JSON.stringify(request));
    const input = ex.vn_alloc(bytes.length);
    let reply = 0;
    try {
      new Uint8Array(ex.memory.buffer, input, bytes.length).set(bytes);
      reply = ex.selftest_call(input, bytes.length);
    } finally {
      ex.vn_free(input, bytes.length);
    }
    const n = new DataView(ex.memory.buffer).getUint32(reply, true);
    const text = new TextDecoder().decode(new Uint8Array(ex.memory.buffer, reply + 4, n));
    ex.vn_free(reply, n + 4);
    return JSON.parse(text);
  }, [ops, hostName]);
}

// One operation; returns its ok value and fails the test on an error or harness message.
export async function call(page, ...op) {
  const [r] = await runOps(page, [op]);
  assert.ok(r && 'ok' in r, `expected ok from ${JSON.stringify(op)}, got ${JSON.stringify(r)}`);
  return r.ok;
}

// One operation expected to fail with HostError kind `kind`; returns the error object.
export async function callErr(page, kind, ...op) {
  const [r] = await runOps(page, [op]);
  assert.ok(r && 'err' in r, `expected ${kind} from ${JSON.stringify(op)}, got ${JSON.stringify(r)}`);
  assert.equal(r.err.kind, kind, `wrong error kind for ${JSON.stringify(op)}: ${JSON.stringify(r.err)}`);
  return r.err;
}

// The DOM under #app, as the page serialises it.
export const appHtml = (page) => page.evaluate(() => document.getElementById('app').innerHTML);

// The handle of #app (the mount point every test uses).
export async function appHandle(page) {
  const handle = await call(page, 'query_selector', '#app');
  assert.equal(typeof handle, 'number', 'the shell must provide #app');
  return handle;
}

// Converts a result to the HostValue shape used by the tests.
export const valueOf = (r) => ({ k: r.k, v: r.v });
