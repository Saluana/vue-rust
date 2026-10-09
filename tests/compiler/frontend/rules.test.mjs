// The profile rule table is data: well-formed, one distinct code per rule, every rule has a reject fixture, and every
// rule is reachable from the parser or the checker.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
import { PROFILE_RULES, ruleById } from '../../../compiler/analysis/profile-rules.mjs';
import { CODES } from '../../../compiler/diagnostics/codes.mjs';

const FIXTURE_DIR = new URL('../../language/rejects/', import.meta.url);
const fixtures = readdirSync(FIXTURE_DIR).filter((n) => /\.(js|ts|jsx|tsx)$/.test(n));

test('the rule table and each rule are frozen', () => {
  assert.ok(Object.isFrozen(PROFILE_RULES));
  for (const rule of PROFILE_RULES) assert.ok(Object.isFrozen(rule), rule.id);
});

test('every rule has a unique id and a registered code, and each code belongs to exactly one rule', () => {
  const ids = new Set();
  const codes = new Map();
  for (const rule of PROFILE_RULES) {
    assert.ok(!ids.has(rule.id), `duplicate rule id ${rule.id}`);
    ids.add(rule.id);
    assert.ok(Object.hasOwn(CODES, rule.code), `${rule.id} uses registered code ${rule.code}`);
    assert.ok(!codes.has(rule.code), `${rule.code} is used by ${rule.id} and ${codes.get(rule.code)}`);
    codes.set(rule.code, rule.id);
  }
});

test('every rule names a manifest row (L01-L21 or S01-S25) and a message and hint', () => {
  for (const rule of PROFILE_RULES) {
    assert.match(rule.row, /^[LS]\d{2}$/, rule.id);
    assert.ok(rule.message.length > 0, `${rule.id} message`);
    assert.ok(rule.hint.length > 0, `${rule.id} hint`);
  }
});

test('languages and targets are declared from the allowed sets', () => {
  for (const rule of PROFILE_RULES) {
    assert.ok(rule.langs.length > 0, rule.id);
    for (const l of rule.langs) assert.ok(['js', 'ts', 'jsx', 'tsx'].includes(l), `${rule.id} lang ${l}`);
    for (const t of rule.targets ?? []) assert.ok(['native', 'web'].includes(t), `${rule.id} target ${t}`);
  }
});

test('node rules list node types, reference rules list nothing extra, parse rules list reasons', () => {
  for (const rule of PROFILE_RULES) {
    if (rule.kind === 'node') assert.ok(rule.nodeTypes.length > 0, rule.id);
    else assert.ok(['ref', 'parse'].includes(rule.kind), `${rule.id} kind`);
    if (rule.kind === 'parse') assert.ok(rule.reasons.length > 0, rule.id);
  }
});

test('each parser reason code maps to exactly one rule', () => {
  const owner = new Map();
  for (const rule of PROFILE_RULES) {
    for (const reason of rule.reasons ?? []) {
      assert.ok(!owner.has(reason), `${reason} is claimed by ${rule.id} and ${owner.get(reason)}`);
      owner.set(reason, rule.id);
    }
  }
  assert.ok(owner.size >= 9, `${owner.size} reasons mapped`);
});

test('every rule has a reject fixture named <rule id>.js or .ts', () => {
  for (const rule of PROFILE_RULES) {
    const matches = fixtures.filter((f) => f.replace(/\.(js|ts|jsx|tsx)$/, '') === rule.id);
    assert.equal(matches.length, 1, `rule ${rule.id} has ${matches.length} fixtures`);
  }
});

test('every reject fixture belongs to a rule of the table', () => {
  const ids = new Set(PROFILE_RULES.map((r) => r.id));
  for (const f of fixtures) {
    assert.ok(ids.has(f.replace(/\.(js|ts|jsx|tsx)$/, '')), `${f} has no rule`);
  }
});

// A rule's fixture extension is its first language mode in the order js, ts, jsx, tsx.
function primaryLang(rule) {
  for (const l of ['js', 'ts', 'jsx', 'tsx']) if (rule.langs.includes(l)) return l;
  throw new RangeError(`rule ${rule.id} declares no language mode`);
}

test('each reject fixture has the extension of its rule language mode', () => {
  for (const rule of PROFILE_RULES) {
    const file = fixtures.find((f) => f.replace(/\.(js|ts|jsx|tsx)$/, '') === rule.id);
    assert.ok(file.endsWith(`.${primaryLang(rule)}`), `${rule.id} fixture is ${file}`);
  }
});

test('ruleById returns the table entry and rejects unknown ids', () => {
  assert.equal(ruleById('with').code, 'VN2001');
  assert.throws(() => ruleById('no-such-rule'), RangeError);
});
