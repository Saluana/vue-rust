#!/usr/bin/env node
// Applies docs/dev/tasks-progress.json to planning/vue-native-framework/tasks.md (idempotent).
// A task is ticked ONLY when its entry has status "done" (the doc's own rule: link a test command + evidence artifact; there is no VCS commit).
// Usage: node tooling/status/update-tasks.mjs [--check]
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const tasksPath = path.join(root, 'planning/vue-native-framework/tasks.md');
const prog = JSON.parse(readFileSync(path.join(root, 'docs/dev/tasks-progress.json'), 'utf8'));
let text = readFileSync(tasksPath, 'utf8');
const LABEL = { done: 'DONE', partial: 'PARTIAL', blocked: 'BLOCKED', notstarted: 'NOT STARTED' };
// 1. strip old annotations
text = text.replace(/\n  <!-- progress:[\d.]+ -->[\s\S]*?<!-- \/progress -->/g, '');
text = text.replace(/\n*<!-- impl-note -->[\s\S]*?<!-- \/impl-note -->\n*/g, '\n\n');
const counts = { done: 0, partial: 0, blocked: 0, notstarted: 0 };
for (const [id, e] of Object.entries(prog.tasks)) {
  const re = new RegExp(`^- \\[[ x]\\] (\\*\\*${id.replace('.', '\\.')} [^\\n]*)$`, 'm');
  const m = text.match(re);
  if (!m) throw new Error(`task ${id} not found in tasks.md`);
  counts[e.status]++;
  const box = e.status === 'done' ? 'x' : ' ';
  const lines = [`  <!-- progress:${id} -->`, `  - **Status (${prog.asOf}): ${LABEL[e.status]}** — ${e.summary}`];
  if (e.evidence) lines.push(`  - **Evidence:** ${e.evidence}`);
  if (e.missing) lines.push(`  - **Not yet done:** ${e.missing}`);
  lines.push('  <!-- /progress -->');
  text = text.replace(re, `- [${box}] ${m[1]}\n${lines.join('\n')}`);
}
const note = `<!-- impl-note -->\n> **Implementation progress (${prog.asOf}).** Annotations below were added during implementation. A box is ticked only when the task's acceptance criteria are met and a test command plus evidence artifact are linked; the workspace is not a version-control repository, so there is no commit to link (evidence = commands that pass in this checkout + artifact paths). Partly finished work is annotated, not ticked. Totals: ${counts.done} done, ${counts.partial} partial, ${counts.blocked} blocked, ${counts.notstarted} not started of ${Object.keys(prog.tasks).length} tasks. Source of truth for the annotations: \`docs/dev/tasks-progress.json\` (regenerate with \`node tooling/status/update-tasks.mjs\`); narrative status in \`docs/dev/STATUS.md\`.\n<!-- /impl-note -->\n\n`;
text = text.replace('## 1. Execution rules and dependencies\n\n', `## 1. Execution rules and dependencies\n\n${note}\n\n`);
if (process.argv.includes('--check')) { const cur = readFileSync(tasksPath, 'utf8'); console.log(cur === text ? 'tasks.md is up to date' : 'tasks.md is STALE'); process.exit(cur === text ? 0 : 1); }
writeFileSync(tasksPath, text);
console.log(`tasks.md updated: ${JSON.stringify(counts)}`);
