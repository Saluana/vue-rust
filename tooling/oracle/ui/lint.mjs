// Lint for UI scenarios (WP-V1). lintUiSource checks one scenario.js; lintUiCase also compiles the directory's .vue files.
//   node tooling/oracle/ui/lint.mjs            lint every case under tests/ui/cases (exit 1 on any problem)
// Rules: the header (line 1 "// @tier N", line 2 "// @covers" with U01..U32, V18..V29 or A01..A19 IDs that exist in the
// manifest); the language rules of tooling/oracle/lint-cases.mjs, except that UI handle methods are allowed; imports only
// from 'vue' names the manifest supports for the native target, and from "./X.vue" files of the same directory; testHost
// members only from the documented set; no reads of Vue internals; every .vue file compiles (compileSfc, native target).
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from '@babel/parser';
import { isSupported, loadManifest } from '../../../compiler/profile/index.mjs';
import { lintSource } from '../lint-cases.mjs';
import { compileCase, listUiCases, uiCasesRoot } from './run-ui.mjs';

// Methods of the NodeHandle and Mount objects (ui-scenario-host.md; tooling/oracle/ui/page-host.mjs).
export const UI_HANDLE_METHODS = new Set(['snapshot', 'html', 'unmount', 'setProps', 'text', 'attr', 'prop', 'exists']);
export const TESTHOST_MEMBERS = new Set(['timers', 'tick', 'mount', 'find', 'findAll', 'click', 'input', 'change', 'check',
  'select', 'keydown', 'focus', 'dispatch', 'container']);
export const TIMER_MEMBERS = new Set(['setTimeout', 'clearTimeout', 'now', 'advance']);
const INTERNAL_MEMBER = /^(__v_\w*|dep|deps|depsTail|flags|globalVersion|effects|cleanups|scopes|_rawValue|_value|_active|_on|_object|_key|_getter|_defaultValue)$/;
const UNMATCHED_METHOD = /^\.([A-Za-z_$][\w$]*)\(\) is not a supported builtin method/;
const TIER = /^\/\/ @tier [123]$/;
const UI_ID = /^[USVA]\d{2}$/;

const manifestRows = new Set(loadManifest().rows.map((r) => r.id));

// Header problems: exactly "// @tier N" then "// @covers ..." as the first two lines, with manifest row IDs.
export function checkUiHeader(source) {
  const lines = source.split('\n');
  const problems = [];
  if (!TIER.test((lines[0] ?? '').trimEnd())) {
    problems.push({ code: 'HEADER', line: 1, message: 'first line must be "// @tier N" with N = 1, 2 or 3' });
  }
  const m = /^\/\/ @covers ([A-Z0-9,]+)$/.exec((lines[1] ?? '').trimEnd());
  if (m === null) {
    problems.push({ code: 'HEADER', line: 2, message: 'second line must be "// @covers U14,V18" with UI row IDs' });
    return problems;
  }
  for (const id of m[1].split(',')) {
    if (!UI_ID.test(id) || !manifestRows.has(id)) {
      problems.push({ code: 'HEADER', line: 2, message: `${id} is not a row of the manifest (U01..U32, S01..S25, V01..V29, A01..A19)` });
    }
  }
  return problems;
}

function* children(node) {
  for (const key of Object.keys(node)) {
    if (['loc', 'start', 'end', 'extra', 'comments', 'leadingComments', 'trailingComments', 'innerComments', 'tokens', 'errors'].includes(key)) continue;
    const v = node[key];
    if (Array.isArray(v)) {
      for (const c of v) if (c && typeof c.type === 'string') yield c;
    } else if (v && typeof v.type === 'string') {
      yield v;
    }
  }
}

// Import and member rules that need the AST. Returns [{code, line, message}].
function astProblems(source, dir) {
  const problems = [];
  let ast;
  try {
    ast = parse(source, { sourceType: 'module' });
  } catch {
    return problems; // lintSource reports PARSE
  }
  const stack = [ast.program];
  while (stack.length > 0) {
    const node = stack.pop();
    const line = node.loc ? node.loc.start.line : 0;
    if (node.type === 'ImportDeclaration') {
      const src = node.source.value;
      if (src === 'vue') {
        for (const s of node.specifiers) {
          const name = s.type === 'ImportSpecifier' ? (s.imported.type === 'Identifier' ? s.imported.name : s.imported.value) : null;
          if (name === null || !isSupported('vue', name, 'native', { origin: 'user' })) {
            problems.push({ code: 'VUE_API', line, message: `"${name ?? s.local.name}" is not a vue export the native profile supports` });
          }
        }
      } else if (/^\.\.?\/.+\.vue$/.test(src)) {
        if (src.startsWith('../')) {
          problems.push({ code: 'IMPORT', line, message: `${src}: components must live in the scenario directory` });
        } else if (!isFileIn(dir, src)) {
          problems.push({ code: 'IMPORT', line, message: `${src} does not exist in the scenario directory` });
        }
      } else {
        problems.push({ code: 'IMPORT', line, message: `"${src}": only 'vue' and "./X.vue" imports are allowed` });
      }
    }
    if (node.type === 'MemberExpression' && !node.computed && node.property.type === 'Identifier') {
      const prop = node.property.name;
      if (node.object.type === 'Identifier' && node.object.name === 'testHost' && !TESTHOST_MEMBERS.has(prop)) {
        problems.push({ code: 'TESTHOST', line, message: `testHost.${prop} is not part of the oracle host` });
      }
      if (node.object.type === 'MemberExpression' && !node.object.computed && node.object.object.type === 'Identifier' &&
          node.object.object.name === 'testHost' && node.object.property.name === 'timers' && !TIMER_MEMBERS.has(prop)) {
        problems.push({ code: 'TESTHOST', line, message: `testHost.timers.${prop} is not part of the fake clock` });
      }
      if (INTERNAL_MEMBER.test(prop)) problems.push({ code: 'VUE_INTERNAL', line, message: `\`.${prop}\` reads Vue internal state` });
    }
    for (const c of children(node)) stack.push(c);
  }
  return problems;
}

function isFileIn(dir, rel) {
  return existsSync(path.join(dir, rel));
}

// Problems of one scenario source; dir is the scenario directory (for the component imports).
export function lintUiSource(source, dir) {
  const problems = checkUiHeader(source);
  for (const p of lintSource(source)) {
    if (p.code === 'HEADER') continue;
    if (p.code === 'IMPORT') continue; // astProblems checks every import
    const m = p.code === 'UNLISTED_API' ? UNMATCHED_METHOD.exec(p.message) : null;
    if (m !== null && UI_HANDLE_METHODS.has(m[1])) continue;
    problems.push(p);
  }
  problems.push(...astProblems(source, dir));
  return problems;
}

// Problems of one scenario directory: the scenario source plus a compile of every component.
export async function lintUiCase(dir) {
  const problems = lintUiSource(await readFile(path.join(dir, 'scenario.js'), 'utf8'), dir).map((p) => ({ ...p, file: 'scenario.js' }));
  try {
    compileCase(dir);
  } catch (e) {
    problems.push({ code: 'COMPILE', line: 0, file: '*.vue', message: String(e.message).split('\n')[0] });
  }
  return problems;
}

const isMain = process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  const dirs = listUiCases(uiCasesRoot);
  let count = 0;
  for (const dir of dirs) {
    for (const p of await lintUiCase(dir)) {
      count += 1;
      process.stdout.write(`${path.relative(uiCasesRoot, dir)}/${p.file}:${p.line} [${p.code}] ${p.message}\n`);
    }
  }
  process.stdout.write(`${dirs.length} case(s), ${count} problem(s)\n`);
  process.exitCode = count === 0 ? 0 : 1;
}
