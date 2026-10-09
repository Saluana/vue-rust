#!/usr/bin/env node
// CLI: node tooling/oracle/run.mjs <scenario.js>  prints the trace document on one line.
import {executeScenario} from './node-runner.mjs';

if (process.argv.length !== 3) {
  process.stderr.write('usage: node tooling/oracle/run.mjs <scenario.js>\n');
  process.exit(2);
}

let printed = false;
// If the event loop empties while run() is still pending, Node exits silently; make that a failure.
process.on('exit', (code) => {
  if (!printed && code === 0) {
    process.stderr.write('scenario did not complete: run() never settled\n');
    process.exitCode = 3;
  }
});

async function main() {
  const doc = await executeScenario(process.argv[2]);
  process.stdout.write(`${JSON.stringify(doc)}\n`);
  printed = true;
}

main().catch((e) => {
  process.stderr.write(`${e.stack ?? e}\n`);
  process.exitCode = 1;
});
