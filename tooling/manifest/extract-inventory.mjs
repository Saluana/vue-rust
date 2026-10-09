#!/usr/bin/env node
// Parses the compatibility tables in design.md sections 8.1-8.5 into one record per row (WP-G0B step 1).
// Fails loudly on any structural mismatch: unexpected headers, column counts, IDs, duplicates, missing rows.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DESIGN_REL, PROFILE_REL, SECTION_LETTER, STATUSES, abs, expectedIds, sha256, toJson } from './common.mjs';

const HEADERS = {
  '8.1': ['ID / construct', 'Status; prototype', 'Contract / implementation', 'Tests and exclusions'],
  '8.2': ['ID / feature', 'Status; prototype', 'Implementation / target behavior', 'T03 acceptance scenario / exclusion'],
  '8.3': ['ID / runtime API', 'Status; prototype', 'Implementation contract', 'Required differential / host case'],
  '8.4': ['ID / feature', 'Status; prototype', 'Native/web behavior and approach', 'Required tests / exclusions'],
  '8.5': ['ID / surface', 'Proposed status and treatment', 'Required case'],
};
// A status letter is a standalone capital P/A/W/X/E (not part of a word such as "Vapor" or "DOM").
const STATUS_RE = /(?<![A-Za-z0-9_$])([PAWXE])(?![A-Za-z0-9_])/g;
const NO_PROTOTYPE = new Set(['—', '–', '-', '']);

const fail = (message) => {
  throw new Error(`extract-inventory: ${message}`);
};

const splitCells = (line) => line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((cell) => cell.trim());

function parseBodyRow(section, cells, line, lineNo) {
  const wide = section !== '8.5';
  if (cells.length !== (wide ? 4 : 3)) fail(`line ${lineNo}: expected ${wide ? 4 : 3} columns, found ${cells.length}`);
  const m = /^([LSVUA])(\d{2})(?: (.+))?$/.exec(cells[0]);
  if (!m) fail(`line ${lineNo}: first cell must start with a row ID such as L01, found "${cells[0]}"`);
  const id = m[1] + m[2];
  if (m[1] !== SECTION_LETTER[section]) fail(`line ${lineNo}: ${id} may not appear in section ${section}`);
  const title = (m[3] ?? '').trim();
  if (!title) fail(`line ${lineNo}: ${id} has an empty title`);
  // Status cell: text before the first ';' is the status; the rest (8.1-8.4 only) is the prototype.
  const statusCell = cells[1];
  const semi = statusCell.indexOf(';');
  const statusText = (semi >= 0 ? statusCell.slice(0, semi) : statusCell).trim();
  let prototype = null;
  if (wide && semi >= 0) {
    const rest = statusCell.slice(semi + 1).trim();
    prototype = NO_PROTOTYPE.has(rest) ? null : rest;
  }
  const statuses = [];
  for (const hit of statusText.matchAll(STATUS_RE)) if (!statuses.includes(hit[1])) statuses.push(hit[1]);
  if (statuses.length === 0) fail(`line ${lineNo}: ${id} has no status letter in "${statusText}"`);
  return {
    id,
    section,
    title,
    statusText,
    statuses,
    primary: statuses[0],
    prototype,
    contract: wide ? cells[2] : cells[1],
    tests: wide ? cells[3] : cells[2],
    rawRow: line,
  };
}

export function extractInventory(text, { source = DESIGN_REL } = {}) {
  const lines = text.split(/\r?\n/);
  const sections = new Map();
  const rows = [];
  let section = null;
  let state = 'idle'; // idle -> header -> body
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const lineNo = i + 1;
    if (line.startsWith('## ') || line.startsWith('### ')) {
      state = 'idle';
      const heading = /^### (8\.\d+) (.*)$/.exec(line);
      section = heading && SECTION_LETTER[heading[1]] ? heading[1] : null;
      if (section) {
        if (sections.has(section)) fail(`section ${section} heading repeated at line ${lineNo}`);
        sections.set(section, { title: heading[2].trim(), tables: 0 });
      }
      continue;
    }
    if (section === null) continue;
    if (!line.startsWith('|')) {
      state = 'idle';
      continue;
    }
    if (line.includes('\\|')) fail(`line ${lineNo}: escaped pipes are not supported in the design tables`);
    const cells = splitCells(line);
    if (state === 'idle') {
      const expected = HEADERS[section];
      if (cells.length !== expected.length || cells.some((cell, j) => cell !== expected[j])) {
        fail(`line ${lineNo}: unexpected header for section ${section}: ${cells.join(' | ')}`);
      }
      sections.get(section).tables += 1;
      state = 'header';
    } else if (state === 'header') {
      if (!/^\|(\s*:?-{3,}:?\s*\|)+\s*$/.test(line)) fail(`line ${lineNo}: separator row missing after header`);
      state = 'body';
    } else {
      rows.push({ ...parseBodyRow(section, cells, line, lineNo), sectionTitle: sections.get(section).title });
    }
  }

  for (const s of Object.keys(SECTION_LETTER)) {
    const info = sections.get(s);
    if (!info) fail(`section ${s} not found`);
    if (info.tables !== 1) fail(`section ${s} must contain exactly one table, found ${info.tables}`);
  }
  const found = new Map();
  for (const row of rows) {
    if (found.has(row.id)) fail(`duplicate row ${row.id}`);
    found.set(row.id, row);
  }
  const expected = expectedIds();
  const expectedSet = new Set(expected);
  const missing = expected.filter((id) => !found.has(id));
  const extra = rows.map((row) => row.id).filter((id) => !expectedSet.has(id));
  if (missing.length || extra.length) {
    fail(`row ID set mismatch: missing [${missing.join(', ')}], unexpected [${extra.join(', ')}], found ${rows.length} rows, expected ${expected.length}`);
  }
  for (const row of rows) {
    for (const s of row.statuses) if (!STATUSES.includes(s)) fail(`${row.id}: invalid status ${s}`);
  }
  return { schemaVersion: 1, source, sourceSha256: sha256(text), rowCount: rows.length, rows };
}

function main(argv) {
  let design = abs(DESIGN_REL);
  let out = abs(path.join(PROFILE_REL, 'inventory.generated.json'));
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--design') design = path.resolve(argv[++i]);
    else if (argv[i] === '--out') out = path.resolve(argv[++i]);
    else throw new Error(`extract-inventory: unknown argument ${argv[i]}`);
  }
  const inventory = extractInventory(readFileSync(design, 'utf8'), { source: DESIGN_REL });
  mkdirSync(path.dirname(out), { recursive: true });
  writeFileSync(out, toJson(inventory));
  console.log(`extract-inventory: wrote ${inventory.rowCount} rows to ${path.relative(process.cwd(), out)}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

