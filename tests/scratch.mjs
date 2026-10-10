// Scratch helpers for the compiler and profile tests (not a test file). Scratch directories live under the work package's
// target directory, as in tests/cli/helpers.mjs, so no test writes into the source tree or the system temp directory.
import fs from 'node:fs';
import path from 'node:path';
import { toolEnv } from '../tooling/cli/build.mjs';

// A fresh, empty directory for one test (an old copy is removed first).
export function scratchDir(name) {
  const dir = path.join(toolEnv().CARGO_TARGET_DIR, 'compiler-tests', name);
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

// A fresh directory holding the given files, { 'src/App.vue': text }, with root-relative names.
export function writeProject(name, files) {
  const dir = scratchDir(name);
  for (const [file, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(dir, file)), { recursive: true });
    fs.writeFileSync(path.join(dir, file), text);
  }
  return dir;
}

// A scratch framework checkout: package.json pins devDependencies, and node_modules holds the installed versions.
export function fakeCheckout(name, { devDependencies, installed }) {
  const dir = scratchDir(`checkout-${name}`);
  fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ name: 'fake', private: true, devDependencies }));
  for (const [pkg, version] of Object.entries(installed)) {
    const pkgDir = path.join(dir, 'node_modules', pkg);
    fs.mkdirSync(pkgDir, { recursive: true });
    fs.writeFileSync(path.join(pkgDir, 'package.json'), JSON.stringify({ name: pkg, version }));
  }
  return dir;
}
