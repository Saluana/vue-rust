// UI oracle runner (WP-V1). runUiScenario(dir) runs one scenario directory in headless Chrome against the real vue@3.5.43
// production build (node_modules/vue/dist/vue.global.prod.js) and resolves with the trace-format document. The .vue files
// of the directory are compiled with compileSfc (target native) before Chrome starts. A loopback HTTP server serves the
// page, the vue global, the 'vue' shim, the compiled components, the scenario module and the page host (tooling/oracle).
import { readFileSync, readdirSync, statSync } from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openPage } from '../../chrome/chrome.mjs';
import { compileSfc, serializeDiagnostics } from '../../../compiler/sfc/index.mjs';
import { loadManifest } from '../../../compiler/profile/index.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
export const repo = path.resolve(here, '../../..');
export const uiCasesRoot = path.join(repo, 'tests/ui/cases');
export const uiGoldenRoot = path.join(repo, 'tests/ui/golden');
const oracleRoot = path.resolve(here, '..');
const vueGlobalFile = path.join(repo, 'node_modules/vue/dist/vue.global.prod.js');
export const VUE_VERSION = '3.5.43';
const JS = 'text/javascript; charset=utf-8';
const byName = (a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);

// Scenario name: the path under tests/ui/cases without a trailing "/scenario.js" (for example "static/elements-text").
export function uiScenarioName(dir) {
  const abs = path.resolve(dir);
  const rel = path.relative(uiCasesRoot, abs);
  if (rel === '' || rel.startsWith('..') || path.isAbsolute(rel)) throw new Error(`${abs} is not under ${uiCasesRoot}`);
  return rel.split(path.sep).join('/');
}

// Every scenario directory (one that holds a scenario.js) under root, in name order.
export function listUiCases(root = uiCasesRoot) {
  const out = [];
  const walk = (d) => {
    for (const ent of readdirSync(d, { withFileTypes: true }).sort(byName)) {
      const p = path.join(d, ent.name);
      if (ent.isDirectory()) walk(p);
      else if (ent.isFile() && ent.name === 'scenario.js') out.push(d);
    }
  };
  walk(root);
  return out;
}

function listVueFiles(dir) {
  const out = [];
  const walk = (d, prefix) => {
    for (const ent of readdirSync(d, { withFileTypes: true }).sort(byName)) {
      const rel = prefix === '' ? ent.name : `${prefix}/${ent.name}`;
      if (ent.isDirectory()) walk(path.join(d, ent.name), rel);
      else if (ent.isFile() && ent.name.endsWith('.vue')) out.push(rel);
    }
  };
  walk(dir, '');
  return out;
}

// Compiles every .vue file of the scenario directory. Returns Map(relative path -> module code); throws on any error.
export function compileCase(dir) {
  const compiled = new Map();
  for (const rel of listVueFiles(dir)) {
    const source = readFileSync(path.join(dir, rel), 'utf8');
    const res = compileSfc(source, { filename: rel, rootDir: dir, target: 'native' });
    if (res.code === null || res.diagnostics.some((d) => d.severity === 'error')) {
      throw new Error(`${rel} does not compile:\n${serializeDiagnostics(res.diagnostics, source, { sources: res.sources })}`);
    }
    compiled.set(rel, res.code);
  }
  return compiled;
}

// The 'vue' module of the page: one named export per vue export of the pinned manifest, each read from the global build.
let shimText = null;
export function vueShimSource() {
  if (shimText === null) {
    const names = [...new Set(loadManifest().vueExports.map((e) => e.name))].sort();
    const lines = [
      '// vue-native UI oracle: the "vue" module of the page. Each export is a member of the global Vue build.',
      'const V = globalThis.Vue;',
      `if (!V || V.version !== ${JSON.stringify(VUE_VERSION)}) throw new Error("vue-native UI oracle: the vue@${VUE_VERSION} global is missing");`,
      'function pick(name) {',
      '  if (!(name in V)) throw new Error("vue-native UI oracle: vue has no export " + name);',
      '  return V[name];',
      '}',
    ];
    for (const n of names) {
      if (!/^[A-Za-z_$][\w$]*$/.test(n)) throw new Error(`vue export ${n} is not an identifier`);
      lines.push(`export const ${n} = pick(${JSON.stringify(n)});`);
    }
    shimText = `${lines.join('\n')}\n`;
  }
  return shimText;
}

let vueGlobal = null;
function vueGlobalText() {
  if (vueGlobal === null) vueGlobal = readFileSync(vueGlobalFile, 'utf8');
  return vueGlobal;
}

const PAGE_HTML = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>vue-native UI oracle</title>
<script src="/vue.global.prod.js"></script>
<script type="importmap">{"imports":{"vue":"/__vue.mjs"}}</script>
</head>
<body></body>
</html>
`;

function isFile(file) {
  try {
    return statSync(file).isFile();
  } catch {
    return false;
  }
}

// Loopback server for one run. Routes: the page, the vue global, the shim, scenario.js, <rel>.vue (compiled) and
// /__oracle/<path>.mjs (tooling/oracle only).
function startServer(dir, compiled) {
  const server = http.createServer((req, res) => {
    if (req.method !== 'GET' && req.method !== 'HEAD') {
      res.writeHead(405, { Allow: 'GET, HEAD' }).end();
      return;
    }
    const send = (type, body) => {
      res.writeHead(200, { 'Content-Type': type, 'Cache-Control': 'no-store' });
      res.end(req.method === 'HEAD' ? undefined : body);
    };
    let p;
    try {
      p = decodeURIComponent(new URL(req.url, 'http://127.0.0.1').pathname);
    } catch {
      res.writeHead(400).end();
      return;
    }
    if (p === '/' || p === '/__page.html') return send('text/html; charset=utf-8', PAGE_HTML);
    if (p === '/vue.global.prod.js') return send(JS, vueGlobalText());
    if (p === '/__vue.mjs') return send(JS, vueShimSource());
    if (p === '/scenario.js') return send(JS, readFileSync(path.join(dir, 'scenario.js')));
    if (p.startsWith('/__oracle/')) {
      const file = path.resolve(oracleRoot, p.slice('/__oracle/'.length));
      if (file.startsWith(oracleRoot + path.sep) && file.endsWith('.mjs') && isFile(file)) {
        return send(JS, readFileSync(file));
      }
    } else if (p.endsWith('.vue') && compiled.has(p.slice(1))) {
      return send(JS, compiled.get(p.slice(1)));
    }
    res.writeHead(404).end();
  });
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      resolve({
        url: `http://127.0.0.1:${port}/`,
        close: () =>
          new Promise((done, fail) => {
            server.close((e) => (e ? fail(e) : done()));
            server.closeAllConnections();
          }),
      });
    });
  });
}

// Runs inside the page (serialized by page.evaluate): loads the page host and runs the scenario in this realm.
async function runInPage(cfg) {
  const host = await import('/__oracle/ui/page-host.mjs');
  return host.runScenario(cfg);
}

// Runs one scenario directory in a fresh Chrome profile and page. Resolves with the trace document (an object).
export async function runUiScenario(dir, { timeout = 30000 } = {}) {
  const scenarioDir = path.resolve(dir);
  const scenario = uiScenarioName(scenarioDir);
  const compiled = compileCase(scenarioDir);
  const server = await startServer(scenarioDir, compiled);
  let page = null;
  try {
    page = await openPage({ viewport: { width: 800, height: 600, dpr: 1 }, timeout });
    await page.goto(`${server.url}__page.html`);
    const line = await page.evaluate(runInPage, { scenario, module: '/scenario.js' });
    return JSON.parse(line);
  } finally {
    if (page !== null) await page.close();
    await server.close();
  }
}
