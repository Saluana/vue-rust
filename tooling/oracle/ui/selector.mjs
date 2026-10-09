// The selector grammar of testHost.find / findAll (ui-scenario-host.md): one compound selector without combinators,
//   [tag] ( #id | .class | [attr] | [attr=value] | [attr="value"] )* [:nth-child(n)]
// Anything else is rejected with a TypeError, so that no scenario relies on selector features the Rust host lacks.

const TAG = /[a-z][a-z0-9-]*/y;
const ID = /#[A-Za-z_][\w-]*/y;
const CLASS = /\.[A-Za-z_][\w-]*/y;
const ATTR = /\[[A-Za-z_][\w:-]*(?:=(?:"[^"\]]*"|'[^'\]]*'|[A-Za-z0-9_-]+))?\]/y;
const NTH = /:nth-child\(\d+\)/y;

// Returns the selector when it is inside the grammar, otherwise throws.
export function checkSelector(selector) {
  if (typeof selector !== 'string' || selector === '') throw new TypeError('selector must be a non-empty string');
  let pos = 0;
  const take = (re) => {
    re.lastIndex = pos;
    const m = re.exec(selector);
    if (m === null) return false;
    pos = re.lastIndex;
    return true;
  };
  take(TAG);
  for (;;) {
    if (pos === selector.length) return selector;
    if (take(ID) || take(CLASS) || take(ATTR)) continue;
    if (take(NTH) && pos === selector.length) return selector;
    break;
  }
  throw new TypeError(`selector "${selector}" is outside the oracle grammar (tag, #id, .class, [attr], [attr=value], :nth-child(n))`);
}
