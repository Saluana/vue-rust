// The ABI contract: the import and export tables of both builds, the version check, rejection of mismatched modules (hand-assembled
// wasm bytes), argument validation of createHost, and isolation of two host instances in one page.
import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { ROOT, call, ensureSite, openHostPage, reloadHost, skipOpt, startSite } from './support.mjs';

// The 45 imports of ABI 1, as hosts/web/ABI.md lists them (hand-written here, not read from the glue).
const IMPORTS = [
  'host_now', 'host_set_timeout', 'host_clear_timeout', 'host_log', 'host_panic', 'host_error_message',
  'create_element', 'create_text', 'create_comment', 'set_text', 'set_element_text', 'insert', 'remove',
  'parent_node', 'next_sibling', 'query_selector', 'body', 'release', 'tag_name', 'is_element',
  'set_attribute', 'set_attribute_ns', 'remove_attribute', 'remove_attribute_ns', 'get_attribute',
  'has_attribute', 'has_property', 'get_property', 'set_property', 'style_set_property', 'style_remove_property',
  'style_get_property', 'computed_style_property', 'style_css_text', 'style_set_css_text', 'class_list_add', 'class_list_remove',
  'add_event_listener', 'remove_event_listener', 'focus', 'blur', 'active_element', 'option_nodes', 'outer_html',
  'inner_html',
];
const EXPORTS = ['memory', 'vn_abi_version', 'vn_init', 'vn_alloc', 'vn_free', 'vn_event', 'vn_timer', 'vn_host_probe'];

// ---- a minimal wasm module, assembled byte by byte (the same LEB128 and section layout as any module)
const leb = (n) => {
  const out = [];
  let v = n;
  do {
    let byte = v & 0x7f;
    v >>>= 7;
    if (v !== 0) byte |= 0x80;
    out.push(byte);
  } while (v !== 0);
  return out;
};
const name = (s) => [...leb(Buffer.byteLength(s)), ...Buffer.from(s, 'utf8')];
const section = (id, body) => [id, ...leb(body.length), ...body];

// `version` is what vn_abi_version returns; `imports` are [module, field] pairs of () -> () functions; `omit` drops an export.
function moduleBytes({ version = 1, imports = [], omit = null } = {}) {
  const types = [[0x60, 0, 1, 0x7f], [0x60, 0, 0], [0x60, 1, 0x7f, 1, 0x7f], [0x60, 2, 0x7f, 0x7f, 0], [0x60, 3, 0x7f, 0x7f, 0x7f, 1, 0x7f]];
  const local = [
    ['vn_abi_version', 0, [0x41, version]],
    ['vn_init', 1, []],
    ['vn_alloc', 2, [0x41, 0]],
    ['vn_free', 3, []],
    ['vn_event', 4, [0x41, 0]],
    ['vn_timer', 2, [0x41, 0]],
  ].filter(([n]) => n !== omit);
  const importSection = imports.flatMap(([m, f]) => [...name(m), ...name(f), 0x00, 1]);
  const bodies = local.map(([, , code]) => {
    const body = [0x00, ...code, 0x0b];
    return [...leb(body.length), ...body];
  });
  const exports = [
    ...local.flatMap(([n], i) => [...name(n), 0x00, ...leb(imports.length + i)]),
    ...name('memory'), 0x02, 0x00,
  ];
  return new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    ...section(1, [types.length, ...types.flat()]),
    ...section(2, [imports.length, ...importSection]),
    ...section(3, [local.length, ...local.map(([, t]) => t)]),
    ...section(5, [1, 0x00, 1]),
    ...section(7, [local.length + 1, ...exports]),
    ...section(10, [local.length, ...bodies.flat()]),
  ]);
}

// Loads bytes into the page's createHost and reports the error text, or 'loaded'.
const tryCreate = (page, bytes) => page.evaluate(async (list) => {
  const { createHost } = await import('./vn-host.js');
  try {
    await createHost({ document, wasmBytesOrUrl: new Uint8Array(list) });
    return 'loaded';
  } catch (e) {
    return `${e.name}: ${e.message}`;
  }
}, [...bytes]);

describe('ABI tables and build outputs', skipOpt, () => {
  let shipped;
  let selftest;
  before(() => {
    const dir = ensureSite();
    const shippedDir = path.join(path.dirname(dir), 'shipped');
    shipped = new WebAssembly.Module(fs.readFileSync(path.join(shippedDir, JSON.parse(fs.readFileSync(path.join(shippedDir, 'manifest.json'))).wasm)));
    selftest = new WebAssembly.Module(fs.readFileSync(path.join(dir, JSON.parse(fs.readFileSync(path.join(dir, 'manifest.json'))).wasm)));
  });

  test('the shipped build imports exactly the 45 vn_host functions of ABI 1', () => {
    const imports = WebAssembly.Module.imports(shipped);
    assert.ok(imports.every((i) => i.module === 'vn_host' && i.kind === 'function'), 'only vn_host functions');
    assert.deepEqual(imports.map((i) => i.name).sort(), [...IMPORTS].sort());
  });

  test('the shipped build exports the ABI 1 entry points and no self-test export', () => {
    const exports = WebAssembly.Module.exports(shipped).map((e) => e.name).sort();
    assert.deepEqual(exports, [...EXPORTS].sort());
  });

  test('the self-test build has the same imports and adds only selftest_call', () => {
    assert.deepEqual(WebAssembly.Module.imports(selftest).map((i) => i.name).sort(), [...IMPORTS].sort());
    assert.deepEqual(WebAssembly.Module.exports(selftest).map((e) => e.name).sort(), [...EXPORTS, 'selftest_call'].sort());
  });

  test('the glue declares ABI_VERSION 1 and its import table names every vn_host function', async () => {
    const glue = await import(new URL('hosts/web/vn-host.js', `file://${ROOT}/`).href);
    assert.equal(glue.ABI_VERSION, 1);
    const source = fs.readFileSync(path.join(ROOT, 'hosts', 'web', 'vn-host.js'), 'utf8');
    for (const name of IMPORTS) assert.match(source, new RegExp(`\\b${name}:`), `missing ${name}`);
  });
});

describe('loading mismatched or invalid modules', skipOpt, () => {
  let site;
  let page;
  before(async () => {
    site = await startSite();
    page = await openHostPage(site);
  });
  after(async () => {
    await page?.close();
    await site?.close();
  });

  test('a module reporting another abiVersion is refused with both versions named', async () => {
    await reloadHost(page, site);
    const result = await tryCreate(page, moduleBytes({ version: 2 }));
    assert.match(result, /ABI mismatch: the module reports abiVersion 2, this glue speaks abiVersion 1/);
  });

  test('a module that imports a vn_host function the glue lacks is refused and the name is given', async () => {
    await reloadHost(page, site);
    const result = await tryCreate(page, moduleBytes({ imports: [['vn_host', 'future_call']] }));
    assert.match(result, /imports vn_host\.future_call, which ABI 1 does not provide/);
  });

  test('an import from another module is refused', async () => {
    await reloadHost(page, site);
    const result = await tryCreate(page, moduleBytes({ imports: [['env', 'host_now']] }));
    assert.match(result, /imports env\.host_now/);
  });

  test('a module missing a required export is refused', async () => {
    await reloadHost(page, site);
    const result = await tryCreate(page, moduleBytes({ omit: 'vn_free' }));
    assert.match(result, /does not export vn_free/);
  });

  test('the same hand-assembled module with the right version and no extras loads', async () => {
    await reloadHost(page, site);
    assert.equal(await tryCreate(page, moduleBytes({ version: 1 })), 'loaded');
  });

  test('bytes that are not WebAssembly fail to compile with CompileError', async () => {
    await reloadHost(page, site);
    const result = await tryCreate(page, new Uint8Array([1, 2, 3, 4]));
    assert.match(result, /^CompileError: /);
  });

  test('createHost validates its arguments', async () => {
    await reloadHost(page, site);
    const errors = await page.evaluate(async () => {
      const { createHost } = await import('./vn-host.js');
      const out = {};
      try { await createHost({ wasmBytesOrUrl: 'x.wasm' }); } catch (e) { out.noDocument = e.name; }
      try { await createHost({ document, wasmBytesOrUrl: 'x.wasm', maxNodes: 0 }); } catch (e) { out.zeroNodes = e.name; }
      try { await createHost({ document, wasmBytesOrUrl: undefined }); } catch (e) { out.noSource = e.name; }
      return out;
    });
    assert.deepEqual(errors, { noDocument: 'TypeError', zeroNodes: 'RangeError', noSource: 'TypeError' });
  });

  test('two host instances in one page keep separate handle tables and wasm memories', async () => {
    await reloadHost(page, site);
    await page.evaluate(async () => {
      const { createHost } = await import('./vn-host.js');
      const manifest = await fetch('./manifest.json').then((r) => r.json());
      window.vnOther = await createHost({ document, wasmBytesOrUrl: new URL(manifest.wasm, location.href) });
    });
    const first = await call(page, 'create_element', 'html', 'q', null);
    const [other] = await page.evaluate(() => [window.vnOther.stats().liveHandles]);
    assert.equal(other, 0, 'the second host has no handles of its own');
    assert.equal(await page.evaluate(() => window.vnOther.memory !== window.vnHost.memory), true);
    assert.equal(first > 0, true);
  });
});

describe('raw exports', skipOpt, () => {
  let site;
  let page;
  before(async () => {
    site = await startSite();
    page = await openHostPage(site);
  });
  after(async () => {
    await page?.close();
    await site?.close();
  });

  test('vn_host_probe links all 45 imports and returns 45', async () => {
    await reloadHost(page, site);
    assert.equal(await page.evaluate(() => window.vnHost.exports.vn_host_probe()), 45);
  });

  test('vn_abi_version returns 1, vn_alloc(0) is non-null and vn_alloc/vn_free round-trip bytes', async () => {
    await reloadHost(page, site);
    const result = await page.evaluate(() => {
      const ex = window.vnHost.exports;
      const zero = ex.vn_alloc(0);
      const p = ex.vn_alloc(16);
      new Uint8Array(ex.memory.buffer, p, 16).set([...Array(16).keys()]);
      const back = [...new Uint8Array(ex.memory.buffer, p, 16)];
      ex.vn_free(p, 16);
      ex.vn_free(zero, 0);
      return { version: ex.vn_abi_version(), zero: zero !== 0, aligned: p % 8 === 0, back };
    });
    assert.deepEqual(result, { version: 1, zero: true, aligned: true, back: [...Array(16).keys()] });
  });
});
