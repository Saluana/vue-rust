#!/usr/bin/env node
// Probe for task 2.2: runs TypeScript 6 and vue-tsc 3.3.12 (the Vue language tools) programmatically on a small project of
// .vue and .ts files, and prints the versions read from node_modules at run time and every diagnostic with its .vue line
// and column. It uses the same code path as `vue-native check` (compiler/types/check.mjs).
//   node tooling/types/probe.mjs [projectDir] [--json]
// Default project: tests/compiler/types/fixtures/pair. Exit 0 with no type errors, 1 with type errors, 2 for a usage error.
// Columns are 0-based UTF-16 units, as in every diagnostic of the CLI.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { TypeCheckError, typeCheckProject } from '../../compiler/types/check.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FRAMEWORK = path.resolve(HERE, '..', '..');
const DEFAULT_PROJECT = path.join(FRAMEWORK, 'tests', 'compiler', 'types', 'fixtures', 'pair');

// Every .vue and .ts file under root (node_modules and dot directories skipped), as sorted root-relative ids ('/src/App.vue').
function listModules(root) {
  const out = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (entry.name === 'node_modules' || entry.name.startsWith('.')) continue;
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (/\.(vue|ts)$/.test(entry.name)) out.push(`/${path.relative(root, full).split(path.sep).join('/')}`);
    }
  };
  walk(root);
  return out.sort();
}

function main(argv) {
  const json = argv.includes('--json');
  const positional = argv.filter((arg) => arg !== '--json');
  if (positional.length > 1 || argv.some((arg) => arg.startsWith('--') && arg !== '--json')) {
    process.stderr.write('usage: node tooling/types/probe.mjs [projectDir] [--json]\n');
    return 2;
  }
  const root = path.resolve(positional[0] ?? DEFAULT_PROJECT);
  if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) {
    process.stderr.write(`probe: no such project directory: ${root}\n`);
    return 2;
  }
  const modules = listModules(root);
  if (modules.length === 0) {
    process.stderr.write(`probe: no .vue or .ts file under ${root}\n`);
    return 2;
  }
  let result;
  try {
    result = typeCheckProject({ root, modules, entry: modules[0], target: 'web', frameworkRoot: FRAMEWORK });
  } catch (err) {
    if (!(err instanceof TypeCheckError)) throw err;
    process.stderr.write(`probe: ${err.message}\n`);
    return 1;
  }
  const errors = result.diagnostics.filter((d) => d.severity === 'error');
  const warnings = result.diagnostics.length - errors.length;
  if (json) {
    process.stdout.write(`${JSON.stringify({
      versions: result.versions,
      modules,
      diagnostics: result.diagnostics.map((d) => ({
        file: d.file,
        line: d.start.line,
        column: d.start.column,
        severity: d.severity,
        code: d.code,
        message: d.message,
      })),
    }, null, 2)}\n`);
  } else {
    const v = result.versions;
    process.stdout.write(`typescript ${v.typescript}, vue-tsc ${v['vue-tsc']}, @vue/language-core ${v['@vue/language-core']}, `
      + `@volar/typescript ${v['@volar/typescript']}, vue ${v.vue} (read from node_modules)\n`);
    process.stdout.write(`checked ${modules.length} module(s): ${modules.join(', ')}\n`);
    for (const d of result.diagnostics) {
      process.stdout.write(`${d.file}:${d.start.line}:${d.start.column}: ${d.severity} ${d.code}: ${d.message.split('\n')[0]}\n`);
    }
    process.stdout.write(`${errors.length} error(s), ${warnings} warning(s)\n`);
  }
  return errors.length > 0 ? 1 : 0;
}

process.exitCode = main(process.argv.slice(2));
