// style/*: CSSOM declarations, cssText parse and serialization, !important, the style attribute (WP-U0).
// Values are chosen so that Chrome does not normalise them further (no shorthands that merge longhands, no colour
// hex or rgb() forms); the known normalisations are listed in crates/dom/src/style.rs.
import { def, UNDEF } from '../builder.mjs';

def('style/set-get-priority', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'color', 'red', false);
  b.op('styleSet', d, 'width', '10px', true);
  b.op('cssText', d);
  b.op('styleGet', d, 'width');
  b.op('stylePriority', d, 'width');
  b.op('stylePriority', d, 'color');
  b.op('getAttribute', d, 'style');
});

def('style/property-name-case-insensitive', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'COLOR', 'blue', false);
  b.op('styleSet', d, 'color', 'green', false);
  b.op('cssText', d);
  b.op('styleGet', d, 'Color');
});

def('style/invalid-values-ignored', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'width', '10px', false);
  b.op('styleSet', d, 'width', 'abc', false);
  b.op('cssText', d);
  b.op('styleSet', d, 'height', 'red', false);
  b.op('cssText', d);
  b.op('styleGet', d, 'height');
});

def('style/update-in-place-and-priority-reset', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'color', 'red', false);
  b.op('styleSet', d, 'width', '10px', true);
  b.op('styleSet', d, 'height', '20px', false);
  b.op('styleSet', d, 'width', '30px', false);
  b.op('cssText', d);
  b.op('stylePriority', d, 'width');
});

def('style/empty-value-removes', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'color', 'red', false);
  b.op('styleSet', d, 'width', '1px', false);
  b.op('styleSet', d, 'color', '', false);
  b.op('cssText', d);
  b.op('getAttribute', d, 'style');
});

def('style/remove-property-returns-old-value', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'height', '20px', false);
  b.op('styleRemove', d, 'height');
  b.op('styleRemove', d, 'height');
  b.op('styleRemove', d, 'nope');
  b.op('cssText', d);
});

def('style/custom-property-case-sensitive', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, '--My-Var', 'Some  Value', false);
  b.op('styleSet', d, '--my-var', 'x', false);
  b.op('styleGet', d, '--My-Var');
  b.op('styleGet', d, '--MY-VAR');
  b.op('cssText', d);
});

def('style/important-serialized-last', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'color', 'red', true);
  b.op('styleSet', d, 'width', '1px', false);
  b.op('cssText', d);
});

def('style/important-group-order', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'color', 'red', true);
  b.op('styleSet', d, 'width', '1px', true);
  b.op('styleSet', d, 'height', '2px', true);
  b.op('styleSet', d, 'top', '3px', false);
  b.op('cssText', d);
});

def('style/cssText-parse-drops-invalid-declaration', (b) => {
  const d = b.el('html', 'div');
  b.op('setCssText', d, 'color red; width: 5px');
  b.op('cssText', d);
});

def('style/cssText-normal-after-important-ignored', (b) => {
  const d = b.el('html', 'div');
  b.op('setCssText', d, 'color: red !important; color: blue');
  b.op('cssText', d);
  b.op('stylePriority', d, 'color');
});

def('style/cssText-later-normal-wins', (b) => {
  const d = b.el('html', 'div');
  b.op('setCssText', d, 'color: red; color: blue');
  b.op('cssText', d);
});

def('style/cssText-important-after-important-wins', (b) => {
  const d = b.el('html', 'div');
  b.op('setCssText', d, 'color: red !important; color: blue !important');
  b.op('cssText', d);
});

def('style/cssText-roundtrip-format', (b) => {
  const d = b.el('html', 'div');
  b.op('setCssText', d, 'color: red; background-color: blue !important;  top:4px;;');
  b.op('cssText', d);
  b.op('getAttribute', d, 'style');
});

def('style/cssText-blank-is-empty', (b) => {
  const d = b.el('html', 'div');
  b.op('setCssText', d, '  ');
  b.op('cssText', d);
  b.op('getAttribute', d, 'style');
});

def('style/attribute-raw-until-cssom-change', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'style', 'top: 1px ;left:2px');
  b.op('getAttribute', d, 'style');
  b.op('cssText', d);
});

def('style/remove-attribute-clears-declarations', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'style', 'color: red');
  b.op('removeAttribute', d, 'style');
  b.op('cssText', d);
  b.op('hasAttribute', d, 'style');
});

def('style/cssom-creates-attribute-at-end', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'data-a', '1');
  b.op('styleSet', d, 'color', 'red', false);
  b.op('attributeNames', d);
  b.op('getAttribute', d, 'style');
});

def('style/lazy-attribute-after-other-attribute', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'color', 'red', false);
  b.op('setAttribute', d, 'data-q', '1');
  b.op('attributeNames', d);
  b.op('styleSet', d, 'opacity', '0.5', false);
  b.op('attributeNames', d);
});

def('style/existing-attribute-keeps-position', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'style', 'color: red');
  b.op('setAttribute', d, 'data-q', '1');
  b.op('styleSet', d, 'color', 'blue', false);
  b.op('attributeNames', d);
  b.op('getAttribute', d, 'style');
});

def('style/set-attribute-after-lazy-cssom', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'color', 'red', false);
  b.op('setAttribute', d, 'data-q', '1');
  b.op('setAttribute', d, 'style', 'top: 1px');
  b.op('attributeNames', d);
  b.op('cssText', d);
});

def('style/remove-absent-property-creates-nothing', (b) => {
  const d = b.el('html', 'div');
  b.op('styleRemove', d, 'color');
  b.op('hasAttribute', d, 'style');
  b.op('styleSet', d, 'color', '', false);
  b.op('attributeNames', d);
});

def('style/keyword-values-lowercased', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'display', 'BLOCK', false);
  b.op('styleGet', d, 'display');
  b.op('styleSet', d, 'color', 'Red', false);
  b.op('cssText', d);
});

def('style/values-trimmed', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'width', '  10px  ', false);
  b.op('styleGet', d, 'width');
  b.op('cssText', d);
});

def('style/length-forms-accepted', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'width', '0', false);
  b.op('styleSet', d, 'height', '50%', false);
  b.op('styleSet', d, 'margin-top', 'auto', false);
  b.op('styleSet', d, 'padding-left', '1.5em', false);
  b.op('styleSet', d, 'max-width', 'none', false);
  b.op('cssText', d);
});

def('style/number-values', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'opacity', '0.5', false);
  b.op('styleSet', d, 'z-index', '3', false);
  b.op('styleSet', d, 'flex-grow', '2', false);
  b.op('cssText', d);
  b.op('styleSet', d, 'z-index', '4', false);
  b.op('cssText', d);
});

def('style/var-value-valid', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'width', 'var(--w)', false);
  b.op('cssText', d);
});

def('style/object-set-syncs-attribute', (b) => {
  const d = b.el('html', 'div');
  b.op('set', d, 'style', 'color: green');
  b.op('getAttribute', d, 'style');
  b.op('cssText', d);
});

def('style/keyword-invalid-ignored', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'display', 'flexy', false);
  b.op('cssText', d);
  b.op('styleSet', d, 'display', 'flex', false);
  b.op('cssText', d);
});

def('style/set-property-undefined-value-text', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'color', 'green', false);
  b.op('set', d, 'style', UNDEF);
  b.op('cssText', d);
});

def('style/camel-case-setter-matches-kebab', (b) => {
  const d = b.el('html', 'div');
  b.op('styleCamel', d, 'backgroundColor', 'red');
  b.op('cssText', d);
  b.op('styleGet', d, 'background-color');
  b.op('styleCamel', d, 'cssFloat', 'left');
  b.op('cssText', d);
  b.op('styleCamel', d, 'marginTop', 'auto');
  b.op('cssText', d);
  b.op('styleCamel', d, 'backgroundColor', '');
  b.op('cssText', d);
});

def('style/camel-case-invalid-value-ignored', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'width', '10px', false);
  b.op('styleCamel', d, 'width', 'abc');
  b.op('cssText', d);
});

def('style/camel-and-kebab-share-declarations', (b) => {
  const d = b.el('html', 'div');
  b.op('styleSet', d, 'border-top-color', 'blue', false);
  b.op('styleCamel', d, 'borderTopColor', 'green');
  b.op('cssText', d);
  b.op('styleGet', d, 'border-top-color');
});
