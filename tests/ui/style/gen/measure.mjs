// Chrome measurements for the value corpora. The fixed context is the one the Rust tests rebuild (crates/ui/tests):
// html (font-size 16px) > body > div#p (font-size 20px, optional font-weight) > div#t (the element under test).
// #t carries BASE (font-size 16px, display none) and the declaration under test, so getComputedStyle returns computed
// values: lengths keep percentages and are otherwise px, as Chrome serialises them for an element without a box.
import { withPage } from '../../../../tooling/chrome/chrome.mjs';

export const BASE = 'font-size: 16px; display: none';
export const VIEWPORT = { width: 800, height: 600, dpr: 1 };
const HTML = '<!doctype html><html style="font-size: 16px"><head></head><body><div id="p" style="font-size: 20px"></div></body></html>';

// In-page: rows are {s: set property, v: value, r: read property, w: parent font-weight or null, typed: read through
// computedStyleMap()}. Chrome returns `none` for transform in getComputedStyle when the element has no box, so transform
// rows read the CSS Typed OM computed value instead (the normalised computed list: translateY(a) is translate(0px, a),
// em and viewport lengths are px, angles are deg). Returns, per row, the base value (no declaration), the value with
// the declaration, and whether CSS.supports accepts the declaration.
export const MEASURE_IN_PAGE = (rows, base) => {
  const p = document.getElementById('p');
  const t = document.createElement('div');
  p.appendChild(t);
  const read = (row) => {
    if (row.typed) {
      const x = t.computedStyleMap().get(row.r);
      return x === null ? '' : String(x);
    }
    return getComputedStyle(t).getPropertyValue(row.r);
  };
  return rows.map((row) => {
    p.style.fontWeight = row.w === null ? '' : String(row.w);
    t.setAttribute('style', base);
    const before = read(row);
    t.setAttribute('style', base + '; ' + row.s + ': ' + row.v);
    const after = read(row);
    return { base: before, value: after, valid: CSS.supports(row.s, row.v) };
  });
};

export async function measureRows(rows) {
  return withPage({ viewport: VIEWPORT }, async (page) => {
    await page.setContent(HTML);
    return page.evaluate(MEASURE_IN_PAGE, rows, BASE);
  });
}
