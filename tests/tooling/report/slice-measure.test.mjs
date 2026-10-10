// Tests of the statistics, the --timing parsing and the state checks of tooling/report/slice-measure.mjs. The expected values
// are worked out by hand in the comments. No test here asserts a timing.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import {
  checkCounterHtml, checkQuickQuoteHtml, counterIteration, parseTimingLine, parseTimingOutput, percentile, quickQuoteIteration, summarize,
} from '../../../tooling/report/slice-measure.mjs';

const TEN = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

describe('percentile: nearest rank', () => {
  test('ten samples: the rank of p is ceil(p * 10 / 100)', () => {
    assert.equal(percentile(TEN, 10), 1); // rank 1
    assert.equal(percentile(TEN, 50), 5); // rank 5
    assert.equal(percentile(TEN, 90), 9); // rank 9
    assert.equal(percentile(TEN, 99), 10); // rank ceil(9.9) = 10
    assert.equal(percentile(TEN, 100), 10); // rank 10
  });

  test('an exact rank stays exact (0.28 * 25 is 7.000000000000001 in floating point)', () => {
    const twentyFive = Array.from({ length: 25 }, (_, i) => i + 1);
    assert.equal(percentile(twentyFive, 28), 7); // rank 28 * 25 / 100 = 7
  });

  test('one sample is every percentile', () => {
    assert.equal(percentile([42], 1), 42);
    assert.equal(percentile([42], 99), 42);
  });

  test('p must be in (0, 100], and there must be samples', () => {
    assert.throws(() => percentile(TEN, 0), RangeError);
    assert.throws(() => percentile(TEN, 101), RangeError);
    assert.throws(() => percentile([], 50), RangeError);
  });

  test('negative control: a wrong percentile expectation is detected', () => {
    assert.throws(() => assert.equal(percentile(TEN, 50), 6), assert.AssertionError);
  });
});

describe('summarize', () => {
  test('min, p50, p90, p99 and max of unsorted samples, by hand for the sorted list 1, 3, 5', () => {
    // p50 rank ceil(1.5) = 2 gives 3; p90 rank ceil(2.7) = 3 gives 5; p99 rank ceil(2.97) = 3 gives 5
    assert.deepEqual(summarize([5, 1, 3]), { n: 3, min: 1, p50: 3, p90: 5, p99: 5, max: 5 });
  });

  test('the summary of ten samples given in reverse order', () => {
    assert.deepEqual(summarize([...TEN].reverse()), { n: 10, min: 1, p50: 5, p90: 9, p99: 10, max: 10 });
  });

  test('negative control: a wrong expected summary is detected', () => {
    assert.throws(() => assert.deepEqual(summarize(TEN), { n: 10, min: 1, p50: 6, p90: 9, p99: 10, max: 10 }), assert.AssertionError);
  });
});

describe('parseTimingLine and parseTimingOutput', () => {
  const step = 'vn-timing {"phase":"step","step":3,"op":"input","selector":"#customer","ns":1500,"collect_ns":700}';

  test('a step line gives its fields', () => {
    assert.deepEqual(parseTimingLine(step), { phase: 'step', step: 3, op: 'input', selector: '#customer', ns: 1500, collect_ns: 700 });
  });

  test('mounted lines may carry null memory figures (no /proc on other systems)', () => {
    const mounted = 'vn-timing {"phase":"mounted","main_to_mounted_ns":2000000,"live_objects":796,"bytes_estimate":155496,'
      + '"allocated_total":924,"vm_rss_kib":null,"vm_hwm_kib":null}';
    assert.equal(parseTimingLine(mounted).main_to_mounted_ns, 2000000);
    assert.equal(parseTimingLine(mounted).vm_hwm_kib, null);
  });

  test('lines without the prefix are not timing lines', () => {
    assert.equal(parseTimingLine('[]'), null);
    assert.equal(parseTimingLine(''), null);
  });

  test('parseTimingOutput keeps the timing lines of mixed stderr, in order', () => {
    const exit = 'vn-timing {"phase":"exit","steps":1,"main_to_end_ns":9000,"live_objects":1,"bytes_estimate":2,"allocated_total":3,'
      + '"vm_rss_kib":4,"vm_hwm_kib":5}';
    assert.deepEqual(parseTimingOutput(`some warning\n${step}\n${exit}\n`).map((t) => t.phase), ['step', 'exit']);
  });

  test('negative control: a malformed or incomplete timing line throws instead of being skipped', () => {
    assert.throws(() => parseTimingLine('vn-timing {"phase":'), /malformed/);
    assert.throws(() => parseTimingLine('vn-timing {"phase":"step","step":3,"op":"input","selector":"#customer","collect_ns":700}'), /"ns"/);
    assert.throws(() => parseTimingLine('vn-timing {"phase":"step","step":3,"op":"hover","selector":"#customer","ns":1,"collect_ns":1}'), /op/);
    assert.throws(() => parseTimingLine('vn-timing {"phase":"nap"}'), /unknown phase/);
  });

  test('negative control: a wrong expected value from a parsed line is detected', () => {
    assert.throws(() => assert.equal(parseTimingLine(step).ns, 1501), assert.AssertionError);
  });
});

describe('workloads and state checks', () => {
  test('a Quick Quote iteration has 20 timed events: 18 inputs (12 typed, a clear, 3 quantity, 2 price) and 2 clicks', () => {
    const steps = quickQuoteIteration();
    assert.equal(steps.length, 20);
    assert.equal(steps.filter((s) => s.op === 'input').length, 18);
    assert.equal(steps.filter((s) => s.op === 'click').length, 2);
  });

  test('a Counter iteration has 7 clicks: six increments and a reset', () => {
    const steps = counterIteration();
    assert.equal(steps.length, 7);
    assert.equal(steps.filter((s) => s.selector === '#increment').length, 6);
    assert.equal(steps.filter((s) => s.selector === '#reset').length, 1);
  });

  const QUOTE = '<main class="quote"><h1>Quick quote</h1><button id="toggle">Hide form</button><section id="editor"></section>'
    + '<ul id="lines"><li class="line" id="line-1"></li><li class="line" id="line-2"></li></ul>'
    + '<p id="total">Total: $29.99</p><p id="status">Quote ready</p></main>';

  test('the Quick Quote state at the start and end of an iteration passes the check', () => {
    assert.deepEqual(checkQuickQuoteHtml(QUOTE), []);
  });

  test('negative control: a wrong total is reported as one mismatch', () => {
    const problems = checkQuickQuoteHtml(QUOTE.replace('Total: $29.99', 'Total: $39.97'));
    assert.equal(problems.length, 1);
    assert.match(problems[0], /#total/);
  });

  test('negative control: a leftover error line and a missing editor are both reported', () => {
    const broken = QUOTE.replace('<section id="editor"></section>', '')
      .replace('<main class="quote">', '<main class="quote"><p class="error">Price must be zero or more dollars.</p>');
    assert.equal(checkQuickQuoteHtml(broken).length, 2);
  });

  const COUNTER = '<main class="counter"><h1>Counter</h1><p id="count">Count: 0</p><p id="doubled">Double: 0</p>'
    + '<button id="increment">Add one</button><button id="reset" disabled="">Reset</button></main>';

  test('the Counter state at the end of an iteration passes the check', () => {
    assert.deepEqual(checkCounterHtml(COUNTER), []);
  });

  test('negative control: a wrong count is reported as one mismatch', () => {
    const problems = checkCounterHtml(COUNTER.replace('Count: 0', 'Count: 1'));
    assert.equal(problems.length, 1);
    assert.match(problems[0], /#count/);
  });

  test('snapshots carry Vue scope stamps in the middle of a tag, and the reading skips them', () => {
    // the runner prints the scope stamp before the other attributes of a tag; the reset button is disabled in this state
    const stamped = COUNTER.replace('<main class="counter">', '<main data-v-7a7a37b1="" class="counter">')
      .replace('<button id="reset" disabled="">', '<button data-v-7a7a37b1="" id="reset" disabled="">');
    assert.notEqual(stamped, COUNTER);
    assert.deepEqual(checkCounterHtml(stamped), []);
    assert.equal(checkCounterHtml(stamped.replace('<button data-v-7a7a37b1="" id="reset" disabled="">', '<button data-v-7a7a37b1="" id="reset">')).length, 1);
  });
});
