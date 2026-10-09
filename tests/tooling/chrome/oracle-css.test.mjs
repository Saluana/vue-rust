import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { chromeSkipReason } from '../../../tooling/chrome/chrome.mjs';
import { computeStyles, matchSelectors, measureBoxes } from '../../../tooling/chrome/oracle-css.mjs';

const skip = chromeSkipReason();
if (skip) process.stderr.write(`[tooling/chrome] SKIPPED oracle-css.test.mjs: ${skip}\n`);
const skipOpt = skip ? { skip } : {}; // Node 26 treats skip: null as a skip, so only set it when needed

const DOCTYPE = '<!doctype html>';

// Layout 1: 300px flex row, 10px gap, three 50x20 items. Hand computed: items start at x = 0, 60, 120.
const ROW = `${DOCTYPE}<html><head><style>
html, body { margin: 0; padding: 0; }
#row { display: flex; gap: 10px; width: 300px; height: 40px; }
#row > div { width: 50px; height: 20px; }
</style></head><body><div id="row"><div id="a"></div><div id="b"></div><div id="c"></div></div></body></html>`;
const ROW_BOXES = [
  { path: 'html>body>div:nth-child(1)', x: 0, y: 0, width: 300, height: 40, top: 0, right: 300, bottom: 40, left: 0 },
  { path: 'html>body>div:nth-child(1)>div:nth-child(1)', x: 0, y: 0, width: 50, height: 20, top: 0, right: 50, bottom: 20, left: 0 },
  { path: 'html>body>div:nth-child(1)>div:nth-child(2)', x: 60, y: 0, width: 50, height: 20, top: 0, right: 110, bottom: 20, left: 60 },
  { path: 'html>body>div:nth-child(1)>div:nth-child(3)', x: 120, y: 0, width: 50, height: 20, top: 0, right: 170, bottom: 20, left: 120 },
];

// Layout 2: content-box 100x50 with 5px/10px padding and a 3px margin. Border box 120x60 at (3, 3).
const PADDED = `${DOCTYPE}<html><head><style>
html, body { margin: 0; padding: 0; }
#box { box-sizing: content-box; width: 100px; height: 50px; padding: 5px 10px; margin: 3px; border: 0; }
</style></head><body><div id="box">x</div></body></html>`;
const PADDED_BOXES = [
  { path: 'html>body>div:nth-child(1)', x: 3, y: 3, width: 120, height: 60, top: 3, right: 123, bottom: 63, left: 3 },
];

// Layout 3: 4px border on a relative 200x100 host. The child's left/top are measured from the host's padding box,
// which starts 4px inside the border: x = 0 + 4 + 10 = 14, y = 0 + 4 + 20 = 24.
const ABS = `${DOCTYPE}<html><head><style>
html, body { margin: 0; padding: 0; }
#host { position: relative; width: 200px; height: 100px; border: 4px solid red; margin: 0; padding: 0; }
#abs { position: absolute; left: 10px; top: 20px; width: 30px; height: 40px; }
</style></head><body><div id="host"><div id="abs"></div></div></body></html>`;
const ABS_BOXES = [
  { path: 'html>body>div:nth-child(1)', x: 0, y: 0, width: 208, height: 108, top: 0, right: 208, bottom: 108, left: 0 },
  {
    path: 'html>body>div:nth-child(1)>div:nth-child(1)',
    x: 14,
    y: 24,
    width: 30,
    height: 40,
    top: 24,
    right: 44,
    bottom: 64,
    left: 14,
  },
];

// Styles: root font-size 10px. #outer 200x80 with font-size 20px. #inner: 2em font (40px), 50% width (100px),
// 25% height of the 80px parent (20px), 10% left padding of the 200px containing block (20px), margin from var(--gap).
const STYLED = `${DOCTYPE}<html style="font-size: 10px"><head><style>
:root { --gap: 12px; --accent: #123456; }
body { margin: 0; color: rgb(1, 2, 3); }
#outer { width: 200px; height: 80px; font-size: 20px; margin: 0; padding: 0; }
#inner { font-size: 2em; width: 50%; height: 25%; padding-left: 10%; margin-left: var(--gap); color: var(--accent); }
#rem { font-size: 1.5rem; }
#fallback { margin-top: var(--missing, 7px); }
</style></head><body>
<div id="outer"><div id="inner"><div id="child"><span id="rem">x</span></div></div></div>
<div id="fallback"></div>
</body></html>`;

const OUTER = 'html>body>div:nth-child(1)';
const INNER = 'html>body>div:nth-child(1)>div:nth-child(1)';
const CHILD = 'html>body>div:nth-child(1)>div:nth-child(1)>div:nth-child(1)';
const REM = 'html>body>div:nth-child(1)>div:nth-child(1)>div:nth-child(1)>span:nth-child(1)';
const FALLBACK = 'html>body>div:nth-child(2)';

// Matching fixture: nine elements. The matchSelectors tests below select subsets, in document order.
const LIST = `${DOCTYPE}<html><head></head><body>
<p id="intro" class="lead">A</p>
<div class="card active" data-role="item">B</div>
<section id="main"><p>C</p><p>D</p></section>
<ul><li>E</li><li class="skip">F</li><li>G</li></ul>
</body></html>`;
const P_INTRO = 'html>body>p:nth-child(1)';
const DIV = 'html>body>div:nth-child(2)';
const SECTION = 'html>body>section:nth-child(3)';
const C = 'html>body>section:nth-child(3)>p:nth-child(1)';
const D = 'html>body>section:nth-child(3)>p:nth-child(2)';
const F = 'html>body>ul:nth-child(4)>li:nth-child(2)';

describe('measureBoxes', () => {
  test('fixed-size flex row with gap matches the hand-computed boxes', skipOpt, async () => {
    assert.deepEqual(await measureBoxes(ROW, '#row, #row > div'), ROW_BOXES);
  });

  test('padded box: border box is content plus padding, offset by the margin', skipOpt, async () => {
    assert.deepEqual(await measureBoxes(PADDED, '#box'), PADDED_BOXES);
  });

  test('absolutely positioned child is offset from the padding box of a bordered relative parent', skipOpt, async () => {
    assert.deepEqual(await measureBoxes(ABS, '#host, #abs'), ABS_BOXES);
  });

  test('negative control: a 12px gap is detected as different from the 10px expectation', skipOpt, async () => {
    const mutated = await measureBoxes(ROW.replace('gap: 10px', 'gap: 12px'), '#row, #row > div');
    assert.equal(mutated[2].x, 62);
    assert.equal(mutated[3].x, 124);
    assert.throws(() => assert.deepEqual(mutated, ROW_BOXES), assert.AssertionError);
  });

  test('empty or non-string selectors are rejected before a browser starts', skipOpt, async () => {
    await assert.rejects(measureBoxes(ROW, []), TypeError);
    await assert.rejects(measureBoxes(ROW, ''), TypeError);
  });
});

describe('computeStyles', () => {
  test('em, rem, percent, var() with fallback, and inheritance resolve to the hand-computed values', skipOpt, async () => {
    assert.deepEqual(
      await computeStyles(STYLED, '#outer, #inner, #child, #rem, #fallback', [
        'font-size',
        'color',
        'margin-top',
        'margin-left',
        'padding-left',
      ]),
      [
        { path: OUTER, styles: { 'font-size': '20px', color: 'rgb(1, 2, 3)', 'margin-top': '0px', 'margin-left': '0px', 'padding-left': '0px' } },
        { path: INNER, styles: { 'font-size': '40px', color: 'rgb(18, 52, 86)', 'margin-top': '0px', 'margin-left': '12px', 'padding-left': '20px' } },
        { path: CHILD, styles: { 'font-size': '40px', color: 'rgb(18, 52, 86)', 'margin-top': '0px', 'margin-left': '0px', 'padding-left': '0px' } },
        { path: REM, styles: { 'font-size': '15px', color: 'rgb(18, 52, 86)', 'margin-top': '0px', 'margin-left': '0px', 'padding-left': '0px' } },
        { path: FALLBACK, styles: { 'font-size': '10px', color: 'rgb(1, 2, 3)', 'margin-top': '7px', 'margin-left': '0px', 'padding-left': '0px' } },
      ],
    );
  });

  test('percentage width and height resolve against the containing block', skipOpt, async () => {
    assert.deepEqual(await computeStyles(STYLED, '#inner, #child', ['width']), [
      { path: INNER, styles: { width: '100px' } },
      { path: CHILD, styles: { width: '100px' } },
    ]);
    assert.deepEqual(await computeStyles(STYLED, '#inner', ['height']), [{ path: INNER, styles: { height: '20px' } }]);
  });

  test('custom properties are readable', skipOpt, async () => {
    const [row] = await computeStyles(STYLED, '#inner', ['--accent']);
    assert.equal(row.styles['--accent'].trim(), '#123456');
  });

  test('negative controls: unknown and camelCase property names are errors', skipOpt, async () => {
    await assert.rejects(computeStyles(STYLED, '#outer', ['font-sizee']), { message: 'unknown CSS property: font-sizee' });
    await assert.rejects(computeStyles(STYLED, '#outer', ['fontSize']), /use the kebab-case CSS name/);
  });
});

describe('matchSelectors', () => {
  test('type selector', skipOpt, async () => {
    assert.deepEqual(await matchSelectors(LIST, 'p'), [P_INTRO, C, D]);
  });

  test('compound class selector', skipOpt, async () => {
    assert.deepEqual(await matchSelectors(LIST, '.card.active'), [DIV]);
  });

  test('id selector', skipOpt, async () => {
    assert.deepEqual(await matchSelectors(LIST, '#main'), [SECTION]);
  });

  test('attribute selector', skipOpt, async () => {
    assert.deepEqual(await matchSelectors(LIST, '[data-role="item"]'), [DIV]);
  });

  test('descendant combinator', skipOpt, async () => {
    assert.deepEqual(await matchSelectors(LIST, 'section p'), [C, D]);
  });

  test('child combinator with nth-child', skipOpt, async () => {
    assert.deepEqual(await matchSelectors(LIST, 'ul > li:nth-child(2)'), [F]);
  });

  test('no match gives an empty array', skipOpt, async () => {
    assert.deepEqual(await matchSelectors(LIST, '.nothing-here'), []);
  });

  test('negative control: an invalid selector is a SyntaxError, not an empty result', skipOpt, async () => {
    await assert.rejects(matchSelectors(LIST, 'div['), (e) => e instanceof SyntaxError);
  });
});
