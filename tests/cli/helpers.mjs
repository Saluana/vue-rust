// Shared helpers for the vue-native CLI tests (WP-APP1). Scratch directories live in this WP's private target directory.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { run } from '../../tooling/cli/cli.mjs';
import { toolEnv } from '../../tooling/cli/build.mjs';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
export const FIXTURES = path.join(ROOT, 'tests', 'cli', 'fixtures');
export const EXAMPLES = path.join(ROOT, 'examples');
export const TEMPLATES = path.join(ROOT, 'templates');

// A scratch path under the target directory that does not exist yet (any old copy is removed first).
export function freshPath(name) {
  const dir = path.join(toolEnv().CARGO_TARGET_DIR, 'cli-tests', name);
  fs.rmSync(dir, { recursive: true, force: true });
  return dir;
}

// A fresh, empty scratch directory.
export function scratch(name) {
  const dir = freshPath(name);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

// Runs the CLI in-process and captures its streams.
export function capture(argv) {
  const out = [];
  const err = [];
  const code = run(argv, { out: { write: (s) => out.push(s) }, err: { write: (s) => err.push(s) } });
  return { code, stdout: out.join(''), stderr: err.join('') };
}
