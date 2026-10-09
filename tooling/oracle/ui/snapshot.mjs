// Canonical element snapshot of the UI oracle (ui-scenario-host.md, "Canonical element snapshot"). The props rule follows
// crates/dom/src/serialize.rs (Document::snapshot_props). Browser-safe: reads DOM properties only, so it runs in the page.

export const HTML_NS = 'http://www.w3.org/1999/xhtml';
const SVG_NS = 'http://www.w3.org/2000/svg';
const MATHML_NS = 'http://www.w3.org/1998/Math/MathML';

// Elements whose `disabled` IDL attribute reflects the content attribute (serialize.rs: Prop::Disabled).
const DISABLED_TAGS = new Set(['button', 'fieldset', 'input', 'optgroup', 'option', 'select', 'textarea']);

// CRLF and CR become LF, as the textarea value API does.
export function normalizeNewlines(s) {
  return s.replace(/\r\n?/g, '\n');
}

// The props of one element: only the properties that differ from the value their attributes give, in the order value,
// checked, selected, disabled. The attribute-derived default is:
//   input value     the value attribute when present, else "on" for checkbox and radio, else ""
//   textarea value  its text content with line breaks normalised
//   checked, selected, disabled   whether the content attribute is present
// value applies to input and textarea only, checked to input, selected to option. A select has no value prop.
export function elementProps(el) {
  const out = [];
  if (el.namespaceURI !== HTML_NS) return out;
  const tag = el.localName;
  if (tag === 'input') {
    const fallback = el.type === 'checkbox' || el.type === 'radio' ? 'on' : '';
    const defaultValue = el.hasAttribute('value') ? el.getAttribute('value') : fallback;
    if (el.value !== defaultValue) out.push(['value', el.value]);
    if (el.checked !== el.hasAttribute('checked')) out.push(['checked', el.checked]);
  } else if (tag === 'textarea') {
    if (el.value !== normalizeNewlines(el.textContent)) out.push(['value', el.value]);
  }
  if (tag === 'option' && el.selected !== el.hasAttribute('selected')) out.push(['selected', el.selected]);
  if (DISABLED_TAGS.has(tag) && el.disabled !== el.hasAttribute('disabled')) out.push(['disabled', el.disabled]);
  return out;
}

// One node of the snapshot: {$:"el",...} with keys in the order tag, [ns], attrs, [props], children; {$:"text"|"comment",v}.
export function snapshotNode(node) {
  if (node.nodeType === 1) return snapshotElement(node);
  if (node.nodeType === 3) return { $: 'text', v: node.data };
  if (node.nodeType === 8) return { $: 'comment', v: node.data };
  throw new TypeError(`snapshot: node type ${node.nodeType} is not part of the oracle snapshot`);
}

function snapshotElement(el) {
  const out = { $: 'el', tag: el.localName };
  if (el.namespaceURI === SVG_NS) out.ns = 'svg';
  else if (el.namespaceURI === MATHML_NS) out.ns = 'math';
  else if (el.namespaceURI !== HTML_NS) throw new TypeError(`snapshot: unsupported namespace ${el.namespaceURI}`);
  out.attrs = [...el.attributes].map((a) => [a.name, a.value]);
  const props = elementProps(el);
  if (props.length > 0) out.props = props;
  out.children = [...el.childNodes].map(snapshotNode);
  return out;
}
