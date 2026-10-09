// Native validation of the template AST (WP-F1 step 4). It runs on the AST as parsed, before the official transforms
// change it: compiler-core appends user nodeTransforms after its own, so a transform would see rewritten nodes and
// expressions. Every element, attribute, directive, modifier and expression is checked; expressions go through the
// profile checks in fragments.mjs.
import { checkFragment, declarationOf, handlerKind, splitFor } from './fragments.mjs';
import { manifestEntry } from './context.mjs';

// Node and tag type numbers of @vue/compiler-core (NodeTypes, ElementTypes), used on the parsed AST.
export const NODE = Object.freeze({ ELEMENT: 1, SIMPLE_EXPRESSION: 4, INTERPOLATION: 5, ATTRIBUTE: 6, DIRECTIVE: 7 });
export const TAG = Object.freeze({ ELEMENT: 0, COMPONENT: 1, SLOT: 2, TEMPLATE: 3 });

// Input types of the native profile (U07, U09, U10). Other static types, and dynamic types, are rejected.
const INPUT_TYPES = new Set(['text', 'checkbox', 'radio', 'range']);

// Directives Vue compiles itself. Any other name is a custom directive and must be bound in <script setup>.
const BUILTIN_DIRECTIVES = new Set([
  'bind', 'on', 'model', 'html', 'text', 'show', 'if', 'else', 'else-if', 'for', 'slot', 'once', 'memo', 'pre', 'cloak',
]);

// Built-in components and elements of Vue's template compiler: lowercase kebab names, checked against the manifest.
const BUILTIN_COMPONENTS = new Set(['component', 'keep-alive', 'transition', 'transition-group', 'teleport', 'suspense']);

function hyphenate(name) {
  return name.replace(/\B([A-Z])/g, '-$1').toLowerCase();
}

function camelize(name) {
  return name.replace(/-(\w)/g, (_, c) => (c ? c.toUpperCase() : ''));
}

function capitalize(name) {
  return name.charAt(0).toUpperCase() + name.slice(1);
}

// Names a component tag may resolve to in <script setup>, as Vue's resolveSetupReference tries them.
function setupCandidates(tag) {
  const camel = camelize(tag);
  return [tag, camel, capitalize(camel)];
}

function modifierName(m) {
  return typeof m === 'string' ? m : m.content;
}

class Walker {
  constructor(ctx, { file, names, scriptBroken, lang, reactiveConsts }) {
    this.ctx = ctx;
    this.file = file;
    this.names = names;
    this.scriptBroken = scriptBroken;
    this.lang = lang;
    this.reactiveConsts = reactiveConsts;
    this.decl = declarationOf(names);
  }

  report(code, start, end, extra) {
    this.ctx.report(code, this.file, start, end, extra);
  }

  // The AST offsets of this file (absolute for the component file, relative to the template text for src files).
  expr(exp, kind) {
    if (!exp || exp.content === undefined) return;
    checkFragment(this.ctx, { file: this.file, base: exp.loc.start.offset, text: exp.content, kind, lang: this.lang, decl: this.decl });
  }

  node(n) {
    if (!n) return;
    if (n.type === NODE.ELEMENT) this.element(n);
    else if (n.type === NODE.INTERPOLATION) {
      if (n.content.content.trim() === '') this.report('VN1000', ...this.span(n.content), { message: 'empty interpolation' });
      else this.expr(n.content, 'expr');
    }
    for (const child of n.children ?? []) this.node(child);
  }

  span(n) {
    return [n.loc.start.offset, n.loc.end.offset];
  }

  element(el) {
    const tagStart = el.loc.start.offset + 1;
    this.tag(el, [tagStart, tagStart + el.tag.length]);
    if (el.tag === 'input') this.inputType(el);
    for (const prop of el.props) {
      if (prop.type === NODE.ATTRIBUTE) this.attribute(el, prop);
      else if (prop.type === NODE.DIRECTIVE) this.directive(el, prop);
    }
  }

  tag(el, span) {
    const tag = el.tag;
    if (el.tagType === TAG.COMPONENT) {
      this.component(tag, span);
      return;
    }
    const st = manifestEntry('native-tag', tag) ?? manifestEntry('element', tag);
    if (!st) {
      this.report('VN6101', ...span, { message: `'<${tag}>' is not a native tag in the profile` });
    } else if (st.status === 'X') {
      this.report('VN6102', ...span, {
        message: `'<${tag}>' is excluded from the native profile`,
        hint: st.note ?? undefined,
      });
    }
  }

  component(tag, span) {
    const kebab = hyphenate(tag);
    if (BUILTIN_COMPONENTS.has(kebab)) {
      const st = manifestEntry('component', kebab);
      if (st && st.status === 'E') {
        this.report('VN6103', ...span, { message: `'<${tag}>' is experimental and not promised for production` });
      }
      return;
    }
    if (kebab === 'base-transition') {
      const st = manifestEntry('vue', 'BaseTransition');
      this.report('VN6104', ...span, {
        message: "'<BaseTransition>' is excluded; only Transition is promised",
        hint: st?.note ?? undefined,
      });
      return;
    }
    if (this.scriptBroken) return;
    if (tag.includes('.') || !setupCandidates(tag).some((c) => this.names.has(c))) {
      const message = /^[a-z]/.test(tag)
        ? `'<${tag}>' is not a native tag in the profile, and custom elements are excluded`
        : `'<${tag}>' is not imported or declared in <script setup>`;
      this.report('VN6101', ...span, { message });
    }
  }

  attribute(el, prop) {
    const st = manifestEntry('attribute', prop.name);
    if (!st) return;
    if (st.status === 'X' || (st.status === 'W' && this.ctx.target !== 'web')) {
      this.report('VN6116', ...this.span(prop), {
        message: `attribute '${prop.name}' is excluded from the native profile`,
        hint: st.note ?? undefined,
      });
    }
  }

  // The type of an input decides which control the profile provides. Checked once per element, so v-model and
  // the type attribute do not report the same problem twice.
  inputType(el) {
    let typeAttr = null;
    let dynamic = null;
    for (const prop of el.props) {
      if (prop.type === NODE.ATTRIBUTE && prop.name === 'type') typeAttr = prop;
      if (prop.type === NODE.DIRECTIVE && prop.name === 'bind' && prop.arg && prop.arg.isStatic && prop.arg.content === 'type') {
        dynamic = prop;
      }
    }
    if (dynamic) {
      this.report('VN6113', ...this.span(dynamic), {
        message: 'a dynamic input type cannot be checked at compile time',
      });
      return;
    }
    const type = typeAttr ? (typeAttr.value?.content ?? '') : 'text';
    if (INPUT_TYPES.has(type)) return;
    const span = typeAttr ? this.span(typeAttr) : this.span(el);
    const hint = type === 'file'
      ? 'Use the explicit file picker control (U10).'
      : type === 'date'
        ? 'Use the explicit date control (U10).'
        : 'Use type text, checkbox, radio or range (U10).';
    this.report('VN6112', ...span, { message: `input type "${type}" is not in the native profile`, hint });
  }

  modifiers(prop, dir) {
    for (const m of prop.modifiers ?? []) {
      const mod = modifierName(m);
      if (dir === 'on' && mod === 'passive') {
        this.report('VN6111', ...this.span(prop), { message: "the '.passive' modifier is excluded from the profile" });
        continue;
      }
      const st = manifestEntry('modifier', `.${mod}`);
      if (!st || st.status === 'X' || (st.status === 'W' && this.ctx.target !== 'web')) {
        this.report('VN6115', ...this.span(prop), {
          message: `modifier '.${mod}' is not supported on v-${dir}`,
          hint: st?.note ?? undefined,
        });
      }
    }
  }

  directive(el, prop) {
    const name = prop.name;
    const span = this.span(prop);
    // Vue compiles :[] to a property named "" instead of rejecting it, so the empty dynamic argument is rejected here.
    if (prop.arg && !prop.arg.isStatic && prop.arg.content.trim() === '') {
      this.report('VN1000', ...this.span(prop.arg), { message: 'empty dynamic argument' });
      return;
    }
    if (!BUILTIN_DIRECTIVES.has(name)) {
      const vName = `v${capitalize(camelize(name))}`;
      if (!this.scriptBroken && !this.names.has(vName)) {
        this.report('VN6114', ...span, {
          message: `custom directive 'v-${name}' is not bound in <script setup> as '${vName}'`,
        });
      }
      if (prop.arg && !prop.arg.isStatic) this.expr(prop.arg, 'expr');
      this.expr(prop.exp, 'expr');
      return;
    }
    if (name === 'bind') {
      if (prop.arg && !prop.arg.isStatic) this.expr(prop.arg, 'expr');
      this.expr(prop.exp, 'expr');
      this.modifiers(prop, 'bind');
    } else if (name === 'on') {
      if (prop.arg && !prop.arg.isStatic) this.expr(prop.arg, 'expr');
      if (prop.exp) {
        const text = prop.exp.content;
        checkFragment(this.ctx, {
          file: this.file,
          base: prop.exp.loc.start.offset,
          text,
          kind: handlerKind(text) === 'expr' ? 'expr' : 'stmts',
          lang: this.lang,
          decl: this.decl,
        });
      }
      this.modifiers(prop, 'on');
    } else if (name === 'model') {
      const target = prop.exp?.content.trim();
      if (target && this.reactiveConsts.has(target)) {
        // Vue rebinds such a const as let so that v-model can write it, and prints a warning. Rejected instead.
        this.report('VN5002', ...this.span(prop.exp), {
          message: `v-model writes '${target}', a const reactive binding that Vue would rebind as let`,
          hint: 'Declare the binding with let, or hold the value in a ref.',
        });
      }
      this.expr(prop.exp, 'expr');
      this.modifiers(prop, 'model');
    } else if (name === 'html') {
      const st = manifestEntry('directive', 'v-html');
      if (this.ctx.target !== 'web' || !this.ctx.trustedHtml) {
        this.report('VN6110', ...span, {
          message: this.ctx.target === 'web'
            ? 'v-html needs the trustedHtml option'
            : 'v-html is excluded from the native profile',
          hint: st?.note ?? undefined,
        });
      }
      this.expr(prop.exp, 'expr');
    } else if (name === 'for') {
      const parts = prop.exp ? splitFor(prop.exp.content) : null;
      if (parts) {
        const base = prop.exp.loc.start.offset;
        checkFragment(this.ctx, { file: this.file, base, text: parts.left, kind: 'vfor-params', lang: this.lang, decl: this.decl });
        checkFragment(this.ctx, { file: this.file, base: base + parts.rightStart, text: parts.right, kind: 'expr', lang: this.lang, decl: this.decl });
      }
    } else if (name === 'slot') {
      if (prop.arg && !prop.arg.isStatic) this.expr(prop.arg, 'expr');
      if (prop.exp) {
        checkFragment(this.ctx, {
          file: this.file,
          base: prop.exp.loc.start.offset,
          text: prop.exp.content,
          kind: 'params',
          lang: this.lang,
          decl: this.decl,
        });
      }
    } else {
      this.expr(prop.exp, 'expr');
    }
  }
}

// Validates the parsed template. ast is the RootNode from parse(); file is the component file or a template src
// file, whose offsets are the AST's own. names are the bindings of <script setup> (component tags and custom
// directives resolve against them). Diagnostics are added to ctx.
export function validateTemplate(ctx, ast, { file, names, scriptBroken, lang, reactiveConsts = new Set() }) {
  const walker = new Walker(ctx, { file, names, scriptBroken, lang, reactiveConsts });
  walker.node(ast);
}
