// vue-native check, create and doctor (WP-APP1). build is in build.mjs.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { findChrome } from '../chrome/chrome.mjs';
import { typeCheckProject } from '../../compiler/types/check.mjs';
import { toolEnv } from './build.mjs';
import { FRAMEWORK_ROOT, UsageError, compileAt, diagnosticJson, formatDiagnostics, resolveProject } from './project.mjs';

export const TEMPLATES = Object.freeze(['counter', 'quick-quote']);
const TARGETS = Object.freeze(['web', 'native']);
const MIN_NODE_MAJOR = 20;

// vue-native check <entry.vue|dir> [--target web|native] [--root <dir>] [--json] [--no-types]
// Runs the SFC, profile and stdlib diagnostics of the whole project, then the type checker (compiler/types/check.mjs,
// TypeScript 6.0.3 with vue-tsc 3.3.12) unless --no-types is given. Exit 0 when there are no errors (warnings are printed).
// The type checker runs only on a project that compiled, as tsc skips its semantic checks after syntax errors: a module
// that failed has no types, and its unresolved imports would only add noise.
// The checks of `check` for one target: the compiler's diagnostics, then the type checker when it can run (types: false
// skips it). vue-native dev runs the same checks before each rebuild.
export function checkProject({ root, entry, target, types = true }) {
  const compiled = compileAt({ root, entry, target });
  let diagnostics = compiled.diagnostics;
  const typeCheck = { ran: false, typescript: null, vueTsc: null };
  if (types && compiled.ok) {
    const result = typeCheckProject({
      root,
      modules: compiled.modules.map((m) => m.id),
      entry: compiled.entryId,
      target,
      frameworkRoot: FRAMEWORK_ROOT,
    });
    diagnostics = [...diagnostics, ...result.diagnostics];
    Object.assign(typeCheck, { ran: true, typescript: result.versions.typescript, vueTsc: result.versions['vue-tsc'] });
  }
  const errors = diagnostics.filter((d) => d.severity === 'error').length;
  return { compiled, diagnostics, typeCheck, errors, warnings: diagnostics.length - errors, ok: errors === 0 };
}

export function check(positionals, values, io) {
  if (positionals.length !== 1) throw new UsageError('check takes one entry .vue file or project directory');
  const target = values.target ?? 'web';
  if (!TARGETS.includes(target)) throw new UsageError(`--target must be web or native, not "${target}"`);
  const { root, entry } = resolveProject(positionals[0], { root: values.root });
  const { compiled, diagnostics, typeCheck, errors, warnings, ok } = checkProject({ root, entry, target, types: !values['no-types'] });
  if (values.json) {
    const report = {
      ok,
      target,
      entry: compiled.entryId,
      modules: compiled.modules.map((m) => ({ id: m.id, lang: m.lang, kind: m.kind, compiled: m.rust !== null })),
      natives: compiled.natives,
      typeCheck,
      diagnostics: diagnostics.map(diagnosticJson),
    };
    io.out.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    if (diagnostics.length > 0) io.err.write(`${formatDiagnostics(diagnostics, compiled.sourceOf)}\n\n`);
    const verdict = ok ? 'ok' : 'failed';
    io.out.write(`${verdict}: ${compiled.modules.length} module(s), ${errors} error(s), ${warnings} warning(s) (${compiled.entryId}, target ${target})\n`);
  }
  return ok ? 0 : 1;
}

// vue-native create <dir> [--template counter|quick-quote]
// Copies a starter project from templates/ into a new directory.
export function create(positionals, values, io) {
  if (positionals.length !== 1) throw new UsageError('create takes one new directory');
  const template = values.template ?? 'counter';
  if (!TEMPLATES.includes(template)) throw new UsageError(`unknown template "${template}" (choose ${TEMPLATES.join(' or ')})`);
  const dir = path.resolve(positionals[0]);
  if (fs.existsSync(dir) && fs.readdirSync(dir).length > 0) {
    throw new UsageError(`${positionals[0]} exists and is not empty; choose a new directory`);
  }
  const source = path.join(FRAMEWORK_ROOT, 'templates', template);
  if (!fs.existsSync(source)) throw new Error(`template "${template}" is missing: ${source}`);
  fs.cpSync(source, dir, { recursive: true, errorOnExist: true, force: false });
  io.out.write(`created ${positionals[0]} from template ${template}\n`);
  io.out.write(`next: vue-native check ${positionals[0]}\n`);
  io.out.write(`then: vue-native build ${positionals[0]} --target web --out <new-dir>\n`);
  return 0;
}

function commandOutput(command, args, env) {
  const r = spawnSync(command, args, { env, encoding: 'utf8' });
  return r.error || r.status !== 0 ? null : r.stdout.trim();
}

// vue-native doctor: reports the toolchain pieces the build needs. It installs nothing.
export function doctor(positionals, values, io) {
  if (positionals.length !== 0) throw new UsageError('doctor takes no arguments');
  const env = toolEnv();
  const checks = [];
  const add = (required, name, ok, detail) => checks.push({ required, name, ok, detail });

  const nodeMajor = Number(process.versions.node.split('.')[0]);
  add(true, `node >= ${MIN_NODE_MAJOR}`, nodeMajor >= MIN_NODE_MAJOR, process.version);
  const runtimeCrate = path.join(FRAMEWORK_ROOT, 'crates', 'runtime', 'Cargo.toml');
  add(true, 'framework checkout', fs.existsSync(runtimeCrate), FRAMEWORK_ROOT);

  const cargo = commandOutput('cargo', ['--version'], env);
  add(true, 'cargo', cargo !== null, cargo ?? 'not found (run from the checkout with toolchain/, or install Rust 1.99.0 with rustup)');
  const rustc = commandOutput('rustc', ['--version'], env);
  let pin = null;
  try {
    pin = /channel\s*=\s*"([^"]+)"/.exec(fs.readFileSync(path.join(FRAMEWORK_ROOT, 'rust-toolchain.toml'), 'utf8'))?.[1] ?? null;
  } catch {
    pin = null;
  }
  add(true, 'rust-toolchain.toml (toolchain pin)', pin !== null, pin ?? 'missing');
  const pinned = rustc !== null && pin !== null && rustc.split(' ')[1] === pin;
  add(true, `rustc ${pin ?? '?'} matches the pin`, pinned, rustc ?? 'not found');
  const sysroot = commandOutput('rustc', ['--print', 'sysroot'], env);
  const wasmLib = sysroot === null ? null : path.join(sysroot, 'lib', 'rustlib', 'wasm32-unknown-unknown');
  const wasmTarget = wasmLib !== null && fs.existsSync(wasmLib);
  add(true, 'wasm32-unknown-unknown target', wasmTarget, wasmTarget ? wasmLib : 'missing: rustup target add wasm32-unknown-unknown');

  let chrome = null;
  try {
    chrome = findChrome();
  } catch (err) {
    chrome = `error: ${err.message}`;
  }
  add(false, 'Google Chrome (only the browser tests need it)', chrome !== null && !chrome.startsWith('error:'), chrome ?? 'not found; set CHROME_PATH');

  for (const c of checks) {
    const mark = c.ok ? '[ok]     ' : c.required ? '[missing]' : '[optional]';
    io.out.write(`${mark} ${c.name}: ${c.detail}\n`);
  }
  const missing = checks.filter((c) => c.required && !c.ok);
  if (missing.length > 0) {
    io.out.write(`${missing.length} required piece(s) missing; nothing was installed\n`);
    return 1;
  }
  io.out.write('the web build toolchain is ready\n');
  return 0;
}
