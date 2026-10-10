// The vue-native command line (WP-APP1). Entry: tooling/cli/vue-native.mjs.
// Exit status: 0 success, 1 the project or the build failed, 2 a usage error. dev runs until it is interrupted, so run
// returns a promise for it (and only for it).
import { parseArgs } from 'node:util';
import { build } from './build.mjs';
import { check, create, doctor } from './commands.mjs';
import { dev } from './dev.mjs';
import { UsageError } from './project.mjs';

export const USAGE = `Usage: vue-native <command> [options]

Commands:
  check <entry.vue|dir> [--target web|native] [--root <dir>] [--json] [--no-types]
      Compile the project, type check it (TypeScript 6.0.3 with vue-tsc 3.3.12) and print its diagnostics.
      Exit 1 when there are errors. --no-types skips the type checker.
  build <entry.vue|dir> --target web --out <new-dir> [--root <dir>] [--name <name>]
      Compile, generate the Rust crate, build it for wasm32-unknown-unknown and package it.
      The output directory must not exist, unless vue-native wrote it before (then it is replaced).
  dev <entry.vue|dir> [--root <dir>] [--no-types] [--no-snapshot] [--json] [--poll-ms <n>] [--name <name>]
      Headless dev loop: runs the app, and on each change to the project checks it, builds it (dev profile), takes a
      snapshot of the root's <script setup> state (refs and plain data), restarts the app and restores the state by
      binding name. Closures and other non-plain values are not kept (named as warnings). A failed check or build keeps
      the running app. --no-snapshot restarts without the state. --json prints one event per line. Ctrl-C stops it.
  create <dir> [--template counter|quick-quote]
      Copy a starter project into a new directory.
  doctor
      Report missing toolchain pieces. Installs nothing.

A directory argument is a project root whose entry is src/App.vue. A .vue file argument is the entry;
its directory is the project root unless --root is given.
Options: -h, --help   show this help
`;

const OPTIONS = {
  help: { type: 'boolean', short: 'h' },
  json: { type: 'boolean' },
  'no-types': { type: 'boolean' },
  'no-snapshot': { type: 'boolean' },
  'poll-ms': { type: 'string' },
  target: { type: 'string' },
  out: { type: 'string' },
  root: { type: 'string' },
  template: { type: 'string' },
  name: { type: 'string' },
};

// Runs one command line. io: { out, err } streams. Returns the exit status (a promise of it for dev).
export function run(argv, io = { out: process.stdout, err: process.stderr }) {
  let parsed;
  try {
    parsed = parseArgs({ args: argv, options: OPTIONS, allowPositionals: true, strict: true });
  } catch (err) {
    io.err.write(`vue-native: ${err.message}\n\n${USAGE}`);
    return 2;
  }
  const { values, positionals } = parsed;
  const [command, ...rest] = positionals;
  if (values.help || command === 'help') {
    io.out.write(USAGE);
    return 0;
  }
  if (command === undefined) {
    io.err.write(USAGE);
    return 2;
  }
  try {
    switch (command) {
      case 'check': return check(rest, values, io);
      case 'build': return build(rest, values, io);
      case 'dev': return dev(rest, values, io);
      case 'create': return create(rest, values, io);
      case 'doctor': return doctor(rest, values, io);
      default: throw new UsageError(`unknown command "${command}"`);
    }
  } catch (err) {
    io.err.write(`vue-native: ${err.message}\n`);
    return err instanceof UsageError ? 2 : 1;
  }
}
