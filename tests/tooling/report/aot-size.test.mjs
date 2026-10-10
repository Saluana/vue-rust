// WP 3.1: the symbol grouping of the release size report (tooling/report/aot-size.mjs). Light tests only: no release build.
// The nm lines below are hand-written in the `nm --size-sort -C --print-size` format; the expected groups are worked out by hand.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { EXAMPLES, crateOf, groupSymbols, parseNm, textOf } from '../../../tooling/report/aot-size.mjs';

const FIXTURE = [
  '0000000000001000 0000000000000010 T vn_value::rt::Rt::new',
  '0000000000001010 0000000000000020 t <vn_value::Value as core::fmt::Debug>::fmt',
  '0000000000002000 0000000000000100 B vn_runtime::static_buffer',
  '0000000000003000 0000000000000030 t core::ptr::drop_in_place<vn_value::Foo>',
  '0000000000004000 0000000000000040 t vn_app_counter_56bf574f::main',
  '0000000000005000 0000000000000004 t memcpy',
  '0000000000006000 0000000000000000 t zero_size_marker',
].join('\n');

describe('aot-size: nm parsing and crate attribution', () => {
  test('parseNm keeps the address-size-type-name lines and their sizes in bytes', () => {
    const syms = parseNm(FIXTURE);
    assert.equal(syms.length, 7);
    assert.deepEqual(syms[0], { size: 16, type: 'T', name: 'vn_value::rt::Rt::new' });
    assert.deepEqual(syms[2], { size: 256, type: 'B', name: 'vn_runtime::static_buffer' });
  });

  test('crateOf names the crate of a path, of a trait impl self type, and of a path-less symbol', () => {
    assert.equal(crateOf('vn_value::rt::Rt::new'), 'vn_value');
    assert.equal(crateOf('<vn_value::Value as core::fmt::Debug>::fmt'), 'vn_value');
    assert.equal(crateOf('core::ptr::drop_in_place<vn_value::Foo>'), 'core');
    assert.equal(crateOf('memcpy'), '(no crate path)');
    // a self type that is a reference has no crate of its own: the trait's crate is used
    assert.equal(crateOf('<&T as core::fmt::Debug>::fmt'), 'core');
  });

  test('crateOf drops the per-build hash of an application crate, and leaves a name without one alone', () => {
    assert.equal(crateOf('vn_app_counter_56bf574f::main'), 'vn_app_counter');
    assert.equal(crateOf('vn_app_counter::main'), 'vn_app_counter');
  });
});

describe('aot-size: symbol grouping (the approximation of the report)', () => {
  test('groups the file-backed symbols by crate, largest first, with ties in name order', () => {
    const g = groupSymbols(FIXTURE);
    // vn_value: 0x10 + 0x20 = 48 bytes; core: 0x30 = 48 bytes; vn_app_counter: 0x40 = 64; no crate path: 4.
    assert.deepEqual(g.groups.map((x) => [x.crate, x.bytes, x.symbols]), [
      ['vn_app_counter', 64, 1],
      ['core', 48, 1],
      ['vn_value', 48, 2],
      ['(no crate path)', 4, 1],
    ]);
    assert.equal(g.totalBytes, 164);
    assert.equal(g.symbols, 5);
  });

  test('negative control: bss (B) and zero-size symbols are not counted, so no runtime group appears', () => {
    const g = groupSymbols(FIXTURE);
    assert.equal(g.groups.some((x) => x.crate === 'vn_runtime'), false);
    assert.notEqual(g.totalBytes, 164 + 256, 'a bss symbol of 256 bytes would change the total');
  });

  test('negative control: a changed symbol type changes the attribution (T counts, t counts, B does not)', () => {
    const asBss = FIXTURE.replace('0000000000001000 0000000000000010 T', '0000000000001000 0000000000000010 B');
    const g = groupSymbols(asBss);
    assert.equal(g.totalBytes, 164 - 16);
    assert.equal(g.groups.find((x) => x.crate === 'vn_value').bytes, 32);
  });

  test('shares add up to one over the counted bytes', () => {
    const g = groupSymbols(FIXTURE);
    const sum = g.groups.reduce((acc, x) => acc + x.share, 0);
    assert.ok(Math.abs(sum - 1) < 1e-9, `shares sum to ${sum}`);
  });
});

describe('aot-size: script output checks', () => {
  test('textOf reads an element text the way the headless tests do', () => {
    assert.equal(textOf('<p id="count">Count: 5</p>', 'count'), 'Count: 5');
    assert.equal(textOf('<p id="count">Count: 5</p>', 'doubled'), null);
  });

  // Quick Quote results as the headless binary prints them: the scope stamp differs per build and must not matter.
  const quickQuote = (total4, status5) => {
    const results = Array.from({ length: 16 }, () => ({ html: '' }));
    results[0] = { html: '<p data-v-7a7a37b1="" id="total">Total: $29.99</p>' };
    results[4] = { html: `<p data-v-7a7a37b1="" id="total">${total4}</p>` };
    results[5] = { html: `<p data-v-7a7a37b1="" id="status">${status5}</p>` };
    results[14] = { html: '<p data-v-7a7a37b1="" id="total">Total: $14.97</p>' };
    results[15] = { html: '<p data-v-7a7a37b1="" id="status">Total changed from $39.97 to $14.97</p>' };
    return results;
  };
  const qq = EXAMPLES.find((e) => e.name === 'quick-quote');

  test('quick-quote check: the expected texts pass, with or without a scope stamp', () => {
    assert.deepEqual(qq.check(quickQuote('Total: $39.97', 'Total changed from $29.99 to $39.97')), []);
  });

  test('negative control: a wrong total (39.98 for 39.97) is reported as a problem', () => {
    const problems = qq.check(quickQuote('Total: $39.98', 'Total changed from $29.99 to $39.97'));
    assert.equal(problems.length, 1);
    assert.match(problems[0], /expected Total: \$39\.97, got Total: \$39\.98/);
  });
});
