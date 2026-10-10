// The pinned type-checker pair (tasks 2.2 and 5.7). TypeScript 6.0.3 is the checker. vue-tsc 3.3.12 is the Vue language
// tools that map .vue files onto that checker, with @vue/language-core 3.3.12 and @volar/typescript 2.4.28, the exact
// versions vue-tsc depends on. TypeScript 7 is not usable: it has no JavaScript compiler API that the Vue language tools
// need (design.md line 573; K07 at line 734), and the manifest records it as excluded (typescript@7, X).
// The installed versions are read from node_modules at run time. A type check never runs with other versions.
//   checkToolingPair(root) -> { ok, versions, problems }   root: the framework checkout (its node_modules)
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const TYPESCRIPT_PIN = '6.0.3';
export const VUE_TSC_PIN = '3.3.12';
export const VUE_PIN = '3.5.43';
export const TOOLING_PAIR = Object.freeze({
  typescript: TYPESCRIPT_PIN,
  'vue-tsc': VUE_TSC_PIN,
  '@vue/language-core': VUE_TSC_PIN,
  '@volar/typescript': '2.4.28',
});
// The framework checkout that contains this file (its package.json and node_modules).
export const FRAMEWORK_DEFAULT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

// The package named `name` as Node resolves it from fromDir: the nearest node_modules/<name>/package.json above it.
// Returns { dir, version } or null. Reading package.json directly avoids "exports" maps that hide it.
export function resolvePackage(name, fromDir) {
  for (let dir = fromDir; ; dir = path.dirname(dir)) {
    const file = path.join(dir, 'node_modules', name, 'package.json');
    if (existsSync(file)) {
      return { dir: path.dirname(file), version: readJson(file).version ?? null };
    }
    if (path.dirname(dir) === dir) return null;
  }
}

// The versions that root actually has installed. vue-tsc's own dependencies are looked up from vue-tsc's directory,
// which is the copy vue-tsc loads.
export function installedVersions(root = FRAMEWORK_DEFAULT) {
  const version = (pkg) => pkg?.version ?? null;
  const vueTsc = resolvePackage('vue-tsc', root);
  return {
    typescript: version(resolvePackage('typescript', root)),
    'vue-tsc': version(vueTsc),
    '@vue/language-core': version(vueTsc && resolvePackage('@vue/language-core', vueTsc.dir)),
    '@volar/typescript': version(vueTsc && resolvePackage('@volar/typescript', vueTsc.dir)),
    vue: version(resolvePackage('vue', root)),
  };
}

// { ok, versions, problems }: ok only when every installed pinned package has its pinned version, and package.json of
// root declares the two direct packages with exact pins (no ranges).
export function checkToolingPair(root = FRAMEWORK_DEFAULT) {
  const versions = installedVersions(root);
  const problems = [];
  for (const [name, pinned] of Object.entries(TOOLING_PAIR)) {
    const got = versions[name];
    if (got === pinned) continue;
    problems.push(name === 'typescript' && typeof got === 'string' && got.split('.')[0] === '7'
      ? `TypeScript ${got} is installed; TypeScript 7 has no JavaScript compiler API for the Vue language tools, use ${pinned}`
      : `${name} ${got ?? 'is not installed'}, pinned ${pinned}`);
  }
  if (versions.vue !== VUE_PIN) problems.push(`vue ${versions.vue ?? 'is not installed'}, pinned ${VUE_PIN}`);
  const declared = existsSync(path.join(root, 'package.json')) ? readJson(path.join(root, 'package.json')).devDependencies ?? {} : {};
  for (const [name, pinned] of [['typescript', TYPESCRIPT_PIN], ['vue-tsc', VUE_TSC_PIN]]) {
    if (declared[name] !== pinned) problems.push(`package.json declares ${name} ${declared[name] ?? 'nothing'}, expected the exact pin ${pinned}`);
  }
  return { ok: problems.length === 0, versions, problems };
}
