// CSS and layout oracle helpers (WP-X1). Each call loads the HTML string verbatim into a fresh headless Chrome at
// 800x600, DPR 1, and reads back what Chrome computes. Element paths look like html>body>div:nth-child(2): html, head
// and body are unique in a document, so they carry no index; every other element carries its 1-based element index.
import { withPage } from './chrome.mjs';

export const ORACLE_VIEWPORT = Object.freeze({ width: 800, height: 600, dpr: 1 });

// 1/64 px is Chrome's layout unit; rounding also turns -0 into 0.
const q = (v) => {
  const r = Math.round(v * 64) / 64;
  return r === 0 ? 0 : r;
};

export function formatPath(chain) {
  return chain
    .map(([tag, index]) => (tag === 'html' || tag === 'head' || tag === 'body' ? tag : `${tag}:nth-child(${index})`))
    .join('>');
}

function selectorList(selectors) {
  const list = Array.isArray(selectors) ? selectors : [selectors];
  if (list.length === 0 || list.some((s) => typeof s !== 'string' || s.trim() === '')) {
    throw new TypeError('selectors must be a non-empty CSS selector string (or an array of them)');
  }
  return list.join(', '); // one selector list: querySelectorAll returns the union in document order
}

// Runs inside the page, so it must be self-contained. mode: 'paths' | 'boxes' | 'styles'.
function queryInPage(sel, mode, props) {
  if (mode === 'styles') {
    for (const p of props) {
      if (!p.startsWith('--') && !CSS.supports(p, 'inherit')) throw new TypeError('unknown CSS property: ' + p);
    }
  }
  return Array.from(document.querySelectorAll(sel), (el) => {
    const chain = [];
    for (let n = el; n; n = n.parentElement) {
      let index = 1;
      for (let s = n.previousElementSibling; s; s = s.previousElementSibling) index++;
      chain.unshift([n.localName, index]);
    }
    if (mode === 'boxes') {
      const r = el.getBoundingClientRect();
      return {
        chain,
        rect: { x: r.x, y: r.y, width: r.width, height: r.height, top: r.top, right: r.right, bottom: r.bottom, left: r.left },
      };
    }
    if (mode === 'styles') {
      const cs = getComputedStyle(el);
      const styles = {};
      for (const p of props) styles[p] = cs.getPropertyValue(p);
      return { chain, styles };
    }
    return { chain };
  });
}

// [{ path, x, y, width, height, top, right, bottom, left }] for every match, in document order. Values are
// getBoundingClientRect() numbers rounded to 1/64 px.
export async function measureBoxes(html, selectors) {
  const sel = selectorList(selectors);
  return withPage({ viewport: ORACLE_VIEWPORT }, async (page) => {
    await page.setContent(html);
    const rows = await page.evaluate(queryInPage, sel, 'boxes', []);
    return rows.map(({ chain, rect }) => ({
      path: formatPath(chain),
      x: q(rect.x),
      y: q(rect.y),
      width: q(rect.width),
      height: q(rect.height),
      top: q(rect.top),
      right: q(rect.right),
      bottom: q(rect.bottom),
      left: q(rect.left),
    }));
  });
}

// [{ path, styles: { [property]: string } }] for every match, in document order. Values are exactly the strings
// getComputedStyle().getPropertyValue() returns (e.g. '40px', 'rgb(18, 52, 86)'). Names must be kebab-case;
// unknown names are an error. Custom properties (--x) are passed through unchecked.
export async function computeStyles(html, selectors, properties) {
  const sel = selectorList(selectors);
  if (!Array.isArray(properties) || properties.length === 0) {
    throw new TypeError('properties must be a non-empty array of CSS property names');
  }
  for (const p of properties) {
    if (typeof p !== 'string' || (!p.startsWith('--') && /[A-Z]/.test(p))) {
      throw new TypeError(`"${p}": use the kebab-case CSS name, e.g. "font-size"`);
    }
  }
  return withPage({ viewport: ORACLE_VIEWPORT }, async (page) => {
    await page.setContent(html);
    const rows = await page.evaluate(queryInPage, sel, 'styles', properties);
    return rows.map(({ chain, styles }) => ({ path: formatPath(chain), styles }));
  });
}

// Paths of every element matching the selector (document order). Invalid selectors reject with a SyntaxError.
export async function matchSelectors(html, selectors) {
  const sel = selectorList(selectors);
  return withPage({ viewport: ORACLE_VIEWPORT }, async (page) => {
    await page.setContent(html);
    const rows = await page.evaluate(queryInPage, sel, 'paths', []);
    return rows.map(({ chain }) => formatPath(chain));
  });
}
