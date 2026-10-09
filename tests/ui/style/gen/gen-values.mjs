// WP-U1b value corpus (tests/ui/style/corpora/values.json). Rows are generated from a fixed seed, then Chrome decides
// every expected value: kind "ok" takes getComputedStyle after the declaration; kind "invalid" (CSS.supports rejects it)
// and kind "unsupported" (valid CSS that the documented subset refuses by design) expect the value without the
// declaration, which is what the cascade does with a dropped declaration. Rows with `pw` set the parent font-weight.
// Run: node tests/ui/style/gen/gen-values.mjs
import { fileURLToPath } from 'node:url';
import { makeRng, writeCorpus } from './common.mjs';
import { measureRows } from './measure.mjs';

export const SEED = 0x7a1e5ca1n;
const NAMED = [
  'aliceblue', 'antiquewhite', 'aqua', 'aquamarine', 'azure', 'beige', 'bisque', 'black', 'blanchedalmond', 'blue', 'blueviolet',
  'brown', 'burlywood', 'cadetblue', 'chartreuse', 'chocolate', 'coral', 'cornflowerblue', 'cornsilk', 'crimson', 'cyan', 'darkblue',
  'darkcyan', 'darkgoldenrod', 'darkgray', 'darkgreen', 'darkgrey', 'darkkhaki', 'darkmagenta', 'darkolivegreen', 'darkorange',
  'darkorchid', 'darkred', 'darksalmon', 'darkseagreen', 'darkslateblue', 'darkslategray', 'darkslategrey', 'darkturquoise',
  'darkviolet', 'deeppink', 'deepskyblue', 'dimgray', 'dimgrey', 'dodgerblue', 'firebrick', 'floralwhite', 'forestgreen', 'fuchsia',
  'gainsboro', 'ghostwhite', 'gold', 'goldenrod', 'gray', 'green', 'greenyellow', 'grey', 'honeydew', 'hotpink', 'indianred',
  'indigo', 'ivory', 'khaki', 'lavender', 'lavenderblush', 'lawngreen', 'lemonchiffon', 'lightblue', 'lightcoral', 'lightcyan',
  'lightgoldenrodyellow', 'lightgray', 'lightgreen', 'lightgrey', 'lightpink', 'lightsalmon', 'lightseagreen', 'lightskyblue',
  'lightslategray', 'lightslategrey', 'lightsteelblue', 'lightyellow', 'lime', 'limegreen', 'linen', 'magenta', 'maroon',
  'mediumaquamarine', 'mediumblue', 'mediumorchid', 'mediumpurple', 'mediumseagreen', 'mediumslateblue', 'mediumspringgreen',
  'mediumturquoise', 'mediumvioletred', 'midnightblue', 'mintcream', 'mistyrose', 'moccasin', 'navajowhite', 'navy', 'oldlace',
  'olive', 'olivedrab', 'orange', 'orangered', 'orchid', 'palegoldenrod', 'palegreen', 'paleturquoise', 'palevioletred',
  'papayawhip', 'peachpuff', 'peru', 'pink', 'plum', 'powderblue', 'purple', 'rebeccapurple', 'red', 'rosybrown', 'royalblue',
  'saddlebrown', 'salmon', 'sandybrown', 'seagreen', 'seashell', 'sienna', 'silver', 'skyblue', 'slateblue', 'slategray',
  'slategrey', 'snow', 'springgreen', 'steelblue', 'tan', 'teal', 'thistle', 'tomato', 'turquoise', 'violet', 'wheat', 'white',
  'whitesmoke', 'yellow', 'yellowgreen',
];

const LENGTH_NUMS = ['0', '1', '2', '3', '10', '0.5', '1.25', '100', '33.333', '1e2', '1.5e1', '4', '-4', '-0.5', '999.9', '12.75', '2.5e-1', '7', '+6'];
const LENGTH_UNITS = ['px', 'px', 'em', 'rem', 'vw', 'vh', 'vmin', 'vmax', '%', 'PX', 'EM', ''];
const LENGTH_PROPS = [
  'width', 'height', 'min-width', 'max-width', 'min-height', 'max-height', 'margin-top', 'margin-left', 'padding-bottom',
  'padding-right', 'top', 'left', 'right', 'bottom', 'letter-spacing', 'outline-offset', 'row-gap', 'column-gap', 'flex-basis',
  'vertical-align', 'border-top-width', 'border-left-width',
];

const KEYWORDS = {
  display: ['block', 'inline', 'inline-block', 'flex', 'inline-flex', 'grid', 'inline-grid', 'none', 'contents', 'table', 'inline-table', 'table-row', 'table-cell', 'list-item', 'flow-root'],
  position: ['static', 'relative', 'absolute', 'fixed', 'sticky'],
  visibility: ['visible', 'hidden', 'collapse'],
  'box-sizing': ['content-box', 'border-box'],
  'overflow-x': ['visible', 'hidden', 'clip', 'scroll', 'auto'],
  'overflow-y': ['visible', 'hidden', 'clip', 'scroll', 'auto'],
  'text-align': ['start', 'end', 'left', 'right', 'center', 'justify'],
  'text-transform': ['none', 'capitalize', 'uppercase', 'lowercase'],
  'white-space': ['normal', 'nowrap', 'pre', 'pre-wrap', 'pre-line', 'break-spaces'],
  'text-overflow': ['clip', 'ellipsis'],
  'overflow-wrap': ['normal', 'break-word', 'anywhere'],
  'word-break': ['normal', 'break-all', 'keep-all', 'break-word'],
  cursor: ['auto', 'default', 'none', 'pointer', 'progress', 'wait', 'cell', 'crosshair', 'text', 'vertical-text', 'alias', 'copy', 'move', 'no-drop', 'not-allowed', 'grab', 'grabbing', 'all-scroll', 'col-resize', 'row-resize', 'n-resize', 'e-resize', 's-resize', 'w-resize', 'ne-resize', 'nw-resize', 'se-resize', 'sw-resize', 'ew-resize', 'ns-resize', 'nesw-resize', 'nwse-resize', 'zoom-in', 'zoom-out', 'help', 'context-menu'],
  'pointer-events': ['auto', 'none'],
  'user-select': ['auto', 'none', 'text', 'all', 'contain'],
  'object-fit': ['fill', 'contain', 'cover', 'none', 'scale-down'],
  'flex-direction': ['row', 'row-reverse', 'column', 'column-reverse'],
  'flex-wrap': ['nowrap', 'wrap', 'wrap-reverse'],
  'justify-content': ['normal', 'stretch', 'center', 'start', 'end', 'flex-start', 'flex-end', 'left', 'right', 'space-between', 'space-around', 'space-evenly'],
  'align-items': ['normal', 'stretch', 'center', 'start', 'end', 'flex-start', 'flex-end', 'self-start', 'self-end', 'baseline'],
  'align-self': ['auto', 'normal', 'stretch', 'center', 'start', 'end', 'flex-start', 'flex-end', 'self-start', 'self-end', 'baseline'],
  'align-content': ['normal', 'stretch', 'center', 'start', 'end', 'flex-start', 'flex-end', 'space-between', 'space-around', 'space-evenly', 'baseline'],
  'justify-items': ['normal', 'stretch', 'center', 'start', 'end', 'flex-start', 'flex-end', 'self-start', 'self-end', 'left', 'right'],
  'justify-self': ['auto', 'normal', 'stretch', 'center', 'start', 'end', 'flex-start', 'flex-end', 'self-start', 'self-end', 'left', 'right'],
  'font-style': ['normal', 'italic', 'oblique'],
  'border-top-style': ['none', 'hidden', 'dotted', 'dashed', 'solid', 'double', 'groove', 'ridge', 'inset', 'outset'],
  'grid-auto-flow': ['row', 'column', 'dense', 'row dense', 'column dense', 'dense row', 'dense column'],
  'text-decoration-line': ['none', 'underline', 'overline', 'line-through', 'blink', 'underline overline', 'underline line-through', 'overline line-through', 'underline overline line-through blink', 'line-through underline', 'blink underline', 'overline underline'],
  'vertical-align': ['baseline', 'sub', 'super', 'text-top', 'text-bottom', 'middle', 'top', 'bottom'],
  'outline-style': ['auto', 'none', 'hidden', 'dotted', 'dashed', 'solid', 'double', 'groove', 'ridge', 'inset', 'outset'],
};
const INVALID_KEYWORDS = ['bogus', 'inlin', 'blocks', '1px', 'initial-value', 'flexs', 'auto-x', 'center-x'];

const COLOUR_PROPS = ['color', 'background-color', 'border-top-color', 'outline-color'];
const FAMILY_WORDS = ['Arial', 'Helvetica', 'Neue', 'Foo', 'Bar', 'Times', 'New', 'Roman', 'Black', 'Mono', 'Serif', 'Gill', 'Sans', 'Ümlaut', 'a1', 'X2', '_x', '-apple-system', 'Bold', 'Narrow', 'Pro'];
const GENERICS = ['serif', 'sans-serif', 'monospace', 'cursive', 'fantasy', 'system-ui', 'ui-monospace', 'math', 'emoji', 'Serif', 'SANS-SERIF', 'Monospace', 'fangsong', 'ui-rounded'];

export function generateRows() {
  const rng = makeRng(SEED);
  const rows = [];
  const add = (c, s, v, r, extra = {}) => rows.push({ c, s, v, r, w: null, u: false, typed: false, ...extra });
  const lenValue = () => rng.pick(LENGTH_NUMS) + rng.pick(LENGTH_UNITS);
  const shuffle = (a) => {
    const out = a.slice();
    for (let i = out.length - 1; i > 0; i--) {
      const j = rng.int(i + 1);
      [out[i], out[j]] = [out[j], out[i]];
    }
    return out;
  };

  // lengths: 700 rows over the length properties.
  for (let i = 0; i < 700; i++) {
    const prop = rng.pick(LENGTH_PROPS);
    add('lengths', prop, lenValue(), prop);
  }
  // colours: every named colour, transparent, currentcolor, and random hex, rgb/hsl (legacy and modern) forms.
  for (const n of NAMED) {
    const prop = rng.pick(COLOUR_PROPS);
    add('colours', prop, n, prop);
  }
  for (const n of ['transparent', 'currentcolor']) add('colours', 'background-color', n, 'background-color');
  for (const prop of COLOUR_PROPS) add('colours', prop, 'currentcolor', prop);
  const HEX = '0123456789abcdef';
  const hex = (k) => Array.from({ length: k }, () => HEX[rng.int(16)]).join('');
  for (let i = 0; i < 200; i++) {
    const prop = rng.pick(COLOUR_PROPS);
    add('colours', prop, '#' + hex(rng.pick([3, 4, 6, 8])), prop);
  }
  const chan = () => rng.pick(['0', '1', '17', '127', '128', '200', '255', '300', '-5', '10.5', '1.5', '2.5', '254.4', '99.5']);
  const pct = () => rng.pick(['0%', '10%', '20%', '50%', '50.5%', '100%', '120%', '-10%', '33%', '2.5%']);
  const alpha = () => rng.pick(['0', '0.1', '0.25', '0.333', '0.5', '0.75', '1', '1.2', '-0.1', '50%', '12.5%', '0.999', '0.0039']);
  for (let i = 0; i < 220; i++) {
    const mode = rng.int(4);
    let v;
    if (mode === 0) v = `rgb(${chan()}, ${chan()}, ${chan()})`;
    else if (mode === 1) v = `rgba(${chan()}, ${chan()}, ${chan()}, ${alpha()})`;
    else if (mode === 2) v = `rgb(${chan()} ${chan()} ${chan()} / ${alpha()})`;
    else v = `rgb(${pct()} ${pct()} ${pct()})`;
    const prop = rng.pick(COLOUR_PROPS);
    add('colours', prop, v, prop);
  }
  for (let i = 0; i < 120; i++) {
    const h = rng.pick(['0', '30', '120', '240', '360', '-30', '720', '45deg', '0.5turn', '1.5rad', '200grad', '99.5']);
    const s = rng.pick(['0%', '10%', '50%', '100%', '120%', '-5%', '33.3%']);
    const l = rng.pick(['0%', '25%', '50%', '75%', '100%', '60%', '-10%']);
    const form = rng.int(3);
    const v = form === 0 ? `hsl(${h}, ${s}, ${l})` : form === 1 ? `hsla(${h}, ${s}, ${l}, ${alpha()})` : `hsl(${h} ${s} ${l} / ${alpha()})`;
    const prop = rng.pick(COLOUR_PROPS);
    add('colours', prop, v, prop);
  }
  for (const v of ['#12', '#12345', '#gggggg', 'rgb(1, 2)', 'rgb(1 2 3 4)', 'red blue', 'notacolour', '#abcdef1']) {
    add('colours', 'color', v, 'color');
  }
  for (const v of ['hwb(120 10% 10%)', 'lab(50% 10 10)', 'oklch(50% 0.1 120)', 'color-mix(in srgb, red 50%, blue)', 'color(srgb 1 0 0)', 'buttonface', 'canvas', 'lch(50% 20 120)']) {
    rows.push({ c: 'colours', s: 'color', v, r: 'color', w: null, u: true });
  }

  // keywords: every keyword of each list, one uppercase variant, and invalid words (ignored).
  for (const [prop, kws] of Object.entries(KEYWORDS)) {
    for (const k of kws) add('keywords', prop, k, prop);
    add('keywords', prop, kws[0].toUpperCase(), prop);
    for (let i = 0; i < 4; i++) add('keywords', prop, rng.pick(INVALID_KEYWORDS), prop);
  }
  for (let i = 0; i < 300; i++) {
    const prop = rng.pick(Object.keys(KEYWORDS));
    add('keywords', prop, rng.pick(KEYWORDS[prop]), prop);
  }
  for (const k of ['inherit', 'initial', 'unset']) {
    for (const prop of ['display', 'color', 'font-size', 'visibility', 'margin-left', 'text-align']) add('keywords', prop, k, prop);
  }

  // shorthands: random 1-4 value boxes and combinations, read through each longhand they set.
  const pick4 = (vals, n) => Array.from({ length: n }, () => rng.pick(vals)).join(' ');
  const boxProps = (prefix) => [`${prefix}-top`, `${prefix}-right`, `${prefix}-bottom`, `${prefix}-left`];
  for (let i = 0; i < 90; i++) {
    const n = rng.range(1, 4);
    add('shorthands', 'margin', pick4(['0', '1px', '2em', 'auto', '10%', '-3px', '1rem', '5px'], n), rng.pick(boxProps('margin')));
    add('shorthands', 'padding', pick4(['0', '1px', '2em', '10%', '5px', '1rem'], n), rng.pick(boxProps('padding')));
    add('shorthands', 'inset', pick4(['auto', '0', '1px', '10%', '-2px'], n), rng.pick(['top', 'right', 'bottom', 'left']));
    add('shorthands', 'border-width', pick4(['0', '1px', 'thin', 'medium', 'thick', '3px', '0.5px'], n), rng.pick(['border-top-width', 'border-right-width', 'border-bottom-width', 'border-left-width']));
    add('shorthands', 'border-style', pick4(['none', 'solid', 'dashed', 'dotted', 'double', 'groove', 'hidden', 'inset'], n), rng.pick(['border-top-style', 'border-right-style', 'border-bottom-style', 'border-left-style']));
    add('shorthands', 'border-color', pick4(['red', '#abc', 'rgb(1 2 3)', 'currentcolor', 'transparent', 'gold'], n), rng.pick(['border-top-color', 'border-right-color', 'border-bottom-color', 'border-left-color']));
    add('shorthands', 'overflow', pick4(['visible', 'hidden', 'scroll', 'auto', 'clip'], rng.range(1, 2)), rng.pick(['overflow-x', 'overflow-y']));
    add('shorthands', 'gap', pick4(['1px', '2px', '10%', 'normal', '5px', '0'], rng.range(1, 2)), rng.pick(['row-gap', 'column-gap']));
  }
  for (let i = 0; i < 90; i++) {
    const w = rng.pick(['1px', 'thin', 'medium', 'thick', '3px', '0', '2em']);
    const st = rng.pick(['solid', 'dashed', 'none', 'dotted', 'double', 'hidden', 'inset']);
    const co = rng.pick(['red', '#123', 'rgb(1, 2, 3)', 'currentcolor', 'transparent', 'blue']);
    const parts = [w, st, co];
    const order = shuffle(parts);
    const take = rng.range(1, 3);
    add('shorthands', 'border', order.slice(0, take).join(' '), rng.pick(['border-top-width', 'border-left-style', 'border-bottom-color', 'border-right-width']));
    add('shorthands', 'border-top', order.slice(0, take).join(' '), rng.pick(['border-top-width', 'border-top-style', 'border-top-color']));
    add('shorthands', 'outline', order.slice(0, take).join(' '), rng.pick(['outline-width', 'outline-style', 'outline-color']));
  }
  for (const v of ['1', '2 3', '0 1 auto', '1 1 10px', '2 3 0%', 'none', 'auto', '20px', '1 0 50px', '0 0 50%', '30% 2', '3 4 0', 'content', '1 1 0']) {
    for (const r of ['flex-grow', 'flex-shrink', 'flex-basis']) add('shorthands', 'flex', v, r);
  }
  for (const v of ['row', 'column wrap', 'wrap-reverse', 'row-reverse nowrap', 'nowrap column']) {
    add('shorthands', 'flex-flow', v, 'flex-direction');
    add('shorthands', 'flex-flow', v, 'flex-wrap');
  }
  for (const v of ['center', 'start end', 'stretch', 'center stretch', 'baseline', 'flex-end space-between'])
    for (const p of ['place-items', 'place-self', 'place-content']) add('shorthands', p, v, p === 'place-items' ? 'justify-items' : p === 'place-self' ? 'justify-self' : 'justify-content');
  for (const v of ['4px', '1px 2px', '1px 2px 3px', '10px / 20px', '1px 2px / 3px 4px 5px', '50%', '2px 4px 6px 8px', '10px 0']) {
    for (const r of ['border-top-left-radius', 'border-top-right-radius', 'border-bottom-right-radius', 'border-bottom-left-radius']) add('shorthands', 'border-radius', v, r);
  }
  for (const v of ['underline', 'none', 'overline line-through', 'underline overline']) add('shorthands', 'text-decoration', v, 'text-decoration-line');
  for (const v of ['1 / 3', '1', 'span 2', 'auto / auto', '2 / span 3', '-1']) {
    add('shorthands', 'grid-column', v, 'grid-column-start');
    add('shorthands', 'grid-column', v, 'grid-column-end');
  }
  for (const v of ['2 / 4', 'span 1 / 3', '3']) {
    add('shorthands', 'grid-row', v, 'grid-row-start');
    add('shorthands', 'grid-row', v, 'grid-row-end');
  }
  for (const v of ['red', 'transparent', 'none', '#fff', 'rgb(10% 20% 30%)']) add('shorthands', 'background', v, 'background-color');
  for (const v of ['12px Arial', 'italic bold 14px/1.5 "Helvetica Neue", sans-serif', 'normal 400 1em serif', 'oblique 700 2em/3 Foo', '30px monospace', 'bold 12px Times, serif']) {
    for (const r of ['font-size', 'font-weight', 'font-style', 'line-height', 'font-family']) add('shorthands', 'font', v, r);
  }
  for (const v of ['url(a.png)', 'linear-gradient(red, blue)', 'url(a.png) no-repeat center']) {
    rows.push({ c: 'shorthands', s: 'background', v, r: 'background-color', w: null, u: true });
  }
  for (const v of ['underline solid', 'underline red 2px', 'small-caps 12px Arial']) {
    rows.push({ c: 'shorthands', s: rng.pick(['text-decoration', 'font']), v, r: 'text-decoration-line', w: null, u: true });
  }
  rows.push({ c: 'shorthands', s: 'background', v: 'none', r: 'background-color', w: null, u: false });
  // border-radius and text-decoration refusals (style and colour parts are not modelled).
  rows.push({ c: 'shorthands', s: 'text-decoration', v: 'underline red', r: 'text-decoration-line', w: null, u: true });
  rows.push({ c: 'shorthands', s: 'text-decoration', v: 'wavy underline', r: 'text-decoration-line', w: null, u: true });

  // calc(): type-correct random expressions (lengths and numbers as CSS types them), some invalid mixes, and min(),
  // max() and clamp() (valid in Chrome, refused by the subset).
  const leafL = () => rng.pick(['1px', '2em', '50%', '10vw', '3rem', '0.5em', '4px', '20%', '5vh', '-2px', '0.25em', '1.5px']);
  const leafN = () => rng.pick(['2', '0.5', '3', '1.5', '-1', '4']);
  const exprT = (depth, want) => {
    if (depth >= 2 || rng.chance(0.35)) return want === 'L' ? leafL() : leafN();
    const wrap = (x) => (rng.chance(0.25) ? `(${x})` : x);
    if (want === 'N') {
      return wrap(`${exprT(depth + 1, 'N')} ${rng.pick(['+', '-', '*', '/'])} ${exprT(depth + 1, 'N')}`);
    }
    switch (rng.int(4)) {
      case 0: return wrap(`${exprT(depth + 1, 'L')} ${rng.pick(['+', '-'])} ${exprT(depth + 1, 'L')}`);
      case 1: return wrap(`${exprT(depth + 1, 'L')} * ${exprT(depth + 1, 'N')}`);
      case 2: return wrap(`${exprT(depth + 1, 'N')} * ${exprT(depth + 1, 'L')}`);
      default: return wrap(`${exprT(depth + 1, 'L')} / ${exprT(depth + 1, 'N')}`);
    }
  };
  const CALC_PROPS = ['width', 'margin-left', 'top', 'letter-spacing', 'padding-left', 'flex-basis', 'row-gap', 'outline-offset', 'height', 'min-width', 'bottom', 'right'];
  for (let i = 0; i < 620; i++) {
    const prop = rng.pick(CALC_PROPS);
    const body = rng.chance(0.06) ? exprT(0, 'N') : exprT(0, 'L');
    add('calc', prop, `calc(${body})`, prop);
  }
  for (let i = 0; i < 30; i++) {
    const prop = rng.pick(['font-size', 'line-height', 'border-top-width', 'padding-top']);
    add('calc', prop, `calc(${exprT(0, 'L')})`, prop);
  }
  for (const v of ['min(10px, 2em)', 'max(1px, 50%)', 'clamp(1px, 5vw, 9px)', 'min(1px + 2px, 4px)', 'calc(min(2px, 3px) + 1px)', 'max(10px, calc(2em - 1px))']) {
    rows.push({ c: 'calc', s: 'width', v: v, r: 'width', w: null, u: true });
  }
  for (const v of ['calc(1px+2px)', 'calc(1px * 2px)', 'calc(1px / 0)', 'calc(0 + 1px)', 'calc(50% + 2)', 'calc(10px - -2px)', 'calc(-(1px))', 'calc(1px / 2)', 'calc(1px / 2px)']) {
    add('calc', 'width', v, 'width');
  }

  // font-family: random lists of unquoted words, quoted names, generic families and invalid items.
  const familyItem = () => {
    const r = rng.int(10);
    if (r < 4) return Array.from({ length: rng.range(1, 3) }, () => rng.pick(FAMILY_WORDS)).join(' ');
    if (r < 6) return `"${rng.pick(['Foo Bar', 'Times New Roman', 'a\\"b', 'Foo  Bar', 'Arial', 'x y z', 'sans-serif', '--foo'])}"`;
    if (r < 7) return `'${rng.pick(['Foo Bar', 'Gill Sans', 'Helvetica Neue'])}'`;
    if (r < 9) return rng.pick(GENERICS);
    return rng.pick(['1foo', 'default', 'inherit', 'x1 2y', '""']);
  };
  for (let i = 0; i < 650; i++) {
    const n = rng.range(1, 4);
    add('font-family', 'font-family', Array.from({ length: n }, familyItem).join(', '), 'font-family');
  }

  // transform: lists of 2D functions with lengths, numbers and angles, read through the typed OM (see measure.mjs);
  // 3D functions and perspective() are refused by the subset.
  const LEN = ['10px', '-5px', '2em', '50%', '1.5rem', '10vw', '0', 'calc(10px + 2em)', 'calc(50% - 4px)', '3vh', '0.5em'];
  const NUM = ['2', '0.5', '-1', '1.5', '3', '0'];
  const ANG = ['45deg', '-30deg', '0.5turn', '1rad', '100grad', '0deg', '90deg', '-0.25turn'];
  const tfFunc = () => {
    switch (rng.int(12)) {
      case 0: return `translate(${rng.pick(LEN)}, ${rng.pick(LEN)})`;
      case 1: return `translate(${rng.pick(LEN)})`;
      case 2: return `translateX(${rng.pick(LEN)})`;
      case 3: return `translateY(${rng.pick(LEN)})`;
      case 4: return `scale(${rng.pick(NUM)}, ${rng.pick(NUM)})`;
      case 5: return `scale(${rng.pick(NUM)})`;
      case 6: return `rotate(${rng.pick(ANG)})`;
      case 7: return `skew(${rng.pick(ANG)}, ${rng.pick(ANG)})`;
      case 8: return `skewX(${rng.pick(ANG)})`;
      case 9: return `matrix(${Array.from({ length: 6 }, () => rng.pick(['1', '0', '2', '-1', '0.5', '10'])).join(', ')})`;
      case 10: return `scaleY(${rng.pick(NUM)})`;
      default: return `skewY(${rng.pick(ANG)})`;
    }
  };
  for (let i = 0; i < 660; i++) {
    const n = rng.range(1, 3);
    add('transform', 'transform', Array.from({ length: n }, tfFunc).join(' '), 'transform', { typed: true });
  }
  for (const v of ['translate(1)', 'rotate(10)', 'scale(2px)', 'translate(10deg)', 'skew(10px)']) add('transform', 'transform', v, 'transform', { typed: true });
  for (const v of ['translate3d(1px, 2px, 3px)', 'perspective(100px)', 'rotateX(10deg)', 'rotate3d(1, 0, 0, 45deg)', 'matrix3d(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)', 'translateZ(5px)', 'scale3d(1, 2, 3)', 'rotateZ(10deg)']) {
    rows.push({ c: 'transform', s: 'transform', v, r: 'transform', w: null, u: true, typed: true });
  }

  // misc: font-size, font-weight (with parent weights), line-height, opacity, z-index, flex numbers, aspect-ratio.
  for (const v of ['xx-small', 'x-small', 'small', 'medium', 'large', 'x-large', 'xx-large', 'xxx-large', 'larger', 'smaller', '2em', '200%', '10px', '0', '-1px', 'xxl', '1.5em', 'calc(1em + 2px)']) {
    add('misc', 'font-size', v, 'font-size');
  }
  for (const pw of [100, 200, 300, 400, 500, 600, 700, 800, 900]) {
    for (const v of ['bolder', 'lighter', 'normal', 'bold', '450.5', '1000', '1001', '0', '1']) add('misc', 'font-weight', v, 'font-weight', { w: pw });
  }
  for (const v of ['normal', '1.5', '2', '0', '-1', '20px', '2em', '50%', 'calc(1em * 1.5)', '1.2px', '10']) add('misc', 'line-height', v, 'line-height');
  for (const v of ['0.5', '1.5', '-1', '50%', '1e-7', '0.1234567', 'auto', '.25']) add('misc', 'opacity', v, 'opacity');
  for (const v of ['-3', '2.5', 'auto', '1e2', '10', '0']) add('misc', 'z-index', v, 'z-index');
  for (const v of ['1', '0.5', '-1', '2.25']) add('misc', 'flex-grow', v, 'flex-grow');
  for (const v of ['auto', '1.5', '16 / 9', '16/9', '0', '2 / 0.5']) add('misc', 'aspect-ratio', v, 'aspect-ratio');
  for (const v of ['thin', 'medium', 'thick', '2px', '0.3px']) add('misc', 'border-top-width', v, 'border-top-width');
  for (const v of ['bold', 'lighter']) add('misc', 'font-weight', v, 'font-weight', { w: 400 });
  return rows;
}

export async function generate(outDir) {
  const rows = generateRows();
  const measured = await measureRows(rows.map((r) => ({ s: r.s, v: r.v, r: r.r, w: r.w, typed: r.typed })));
  const out = rows.map((r, i) => {
    const m = measured[i];
    let kind;
    let expected;
    if (r.u) {
      kind = 'unsupported';
      expected = m.base;
    } else if (!m.valid) {
      kind = 'invalid';
      expected = m.base;
    } else {
      kind = 'ok';
      expected = m.value;
    }
    return { c: r.c, s: r.s, v: r.v, r: r.r, w: r.w, typed: r.typed, k: kind, e: expected };
  });
  const counts = {};
  for (const r of out) counts[r.c] = (counts[r.c] || 0) + 1;
  const doc = {
    about: 'WP-U1b value corpus: Chrome computed values for (declaration, read property) rows in the fixed context of crates/ui/tests/values_corpus.rs',
    seed: String(SEED),
    context: 'html font-size 16px > div#p (font-size 20px, font-weight w when set) > div#t (style: font-size 16px; display: none; then the declaration)',
    viewport: { width: 800, height: 600 },
    rows: out,
  };
  const file = writeCorpus('values.json', doc, outDir);
  return { file, counts, total: out.length };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const result = await generate();
  console.log(JSON.stringify(result.counts), 'total', result.total, result.file);
}
