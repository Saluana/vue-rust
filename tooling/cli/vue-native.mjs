#!/usr/bin/env node
// The vue-native command line (WP-APP1): node tooling/cli/vue-native.mjs <command> [options]. See --help.
import { run } from './cli.mjs';

process.exitCode = run(process.argv.slice(2));
