// Real-Vue execution oracle (WP-F1). Each accepting fixture is compiled by compileSfc, erased of TypeScript when
// lang="ts", and mounted in headless Chrome with the browser build of vue@3.5.43 (node_modules/vue/dist/vue.global.prod.js,
// inlined into the page). The expected HTML and computed styles are written by hand from Vue's rendering rules, not
// produced by the compiler. Interactions (click, input, change, select) run inside the page and the result is awaited
// after Vue's scheduler has flushed. Without Chrome every test is skipped and a line on stderr says why.
import { test, describe, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { compileSfc } from '../../../compiler/sfc/index.mjs';
import { chromeSkipReason, openPage } from '../../../tooling/chrome/chrome.mjs';
import { isAlive } from '../../tooling/chrome/support.mjs';
import { FIXTURES, pageFor } from './harness.mjs';

const skip = chromeSkipReason();
if (skip) process.stderr.write(`[compiler/sfc] SKIPPED oracle.test.mjs: ${skip}\n`);
const skipOpt = skip ? { skip } : {}; // Node 26 treats skip: null as a skip, so only set it when needed

const H = '{ render() { return Vue.h(comp) } }';
const html = 'window.__vn.html()';
const tick = 'window.__vn.tick()';
const color = (sel) => `getComputedStyle(document.querySelector('${sel}')).color`;

// wrapper: root component options (Vue and comp in scope). steps: [{ run, expect }] evaluated in order.
const CASES = [
  {
    file: 'ok-counter.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<button type="button">0</button>' },
      { run: "window.__vn.click('button')", expect: '<button type="button">1</button>' },
    ],
  },
  {
    file: 'ok-props-emits.vue',
    wrapper: "{ render() { return Vue.h(comp, { label: 'hi', onPing: (n) => { window.__pings = (window.__pings || []).concat([n]) } }) } }",
    steps: [
      { run: html, expect: '<div><button class="ping">hi 0</button></div>' },
      { run: "window.__vn.click('button.ping')", expect: '<div><button class="ping">hi 1</button></div>' },
      { run: 'JSON.stringify(window.__pings)', expect: '[1]' },
    ],
  },
  {
    file: 'ok-define-model.vue',
    wrapper: "{ render() { return Vue.h(comp, { modelValue: 'x', 'onUpdate:modelValue': (v) => { window.__got = v } }) } }",
    steps: [
      // :value writes the value attribute at mount and only the property on update. A hand-written h() component
      // with the same bindings gives the same markup (checked while writing this test).
      { run: html, expect: '<input type="text" value="x">' },
      { run: "window.__vn.input('input', 'typed')", expect: '<input type="text" value="x">' },
      { run: 'window.__got', expect: 'typed' },
    ],
  },
  {
    file: 'ok-slots.vue',
    wrapper: "{ render() { return Vue.h(comp, null, { title: () => Vue.h('b', 'T'), default: () => Vue.h('i', 'body') }) } }",
    steps: [{ run: html, expect: '<section><header><b>T</b></header><main><i>body</i></main></section>' }],
  },
  {
    file: 'ok-scoped-slots.vue',
    wrapper: "{ render() { return Vue.h(comp, null, { default: (p) => Vue.h('em', p.index + ':' + p.item) }) } }",
    steps: [{ run: html, expect: '<ul><li><em>0:a</em></li><li><em>1:b</em></li></ul>' }],
  },
  {
    file: 'ok-v-for-key.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<div><button>shuffle</button><p>x</p><p>y</p></div>' },
      { run: "window.__vn.click('button')", expect: '<div><button>shuffle</button><p>y</p><p>x</p></div>' },
    ],
  },
  {
    file: 'ok-v-if-else.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<div><button>toggle</button><span>yes</span><em>e</em></div>' },
      { run: "window.__vn.click('button')", expect: '<div><button>toggle</button><span>no</span><!--v-if--></div>' },
    ],
  },
  {
    file: 'ok-v-model-text.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<div><input type="text"><p>start</p></div>' },
      { run: "window.__vn.input('input', 'hello')", expect: '<div><input type="text"><p>hello</p></div>' },
    ],
  },
  {
    file: 'ok-v-model-checkbox.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<div><input type="checkbox"><p>no</p></div>' },
      { run: "window.__vn.change('input', true)", expect: '<div><input type="checkbox"><p>yes</p></div>' },
    ],
  },
  {
    file: 'ok-v-model-select.vue',
    wrapper: H,
    steps: [
      {
        run: html,
        expect: '<div><select><option value="red">red</option><option value="blue">blue</option></select><p>red</p></div>',
      },
      {
        run: "window.__vn.select('select', 'blue')",
        expect: '<div><select><option value="red">red</option><option value="blue">blue</option></select><p>blue</p></div>',
      },
    ],
  },
  {
    file: 'ok-with-defaults.vue',
    wrapper: '{ render() { return Vue.h(comp, { n: 5 }) } }',
    steps: [{ run: html, expect: '<p>em 5</p>' }],
  },
  {
    file: 'ok-ts-generic.vue',
    wrapper: "{ render() { return Vue.h(comp, { items: ['p', 'q'] }) } }",
    steps: [{ run: html, expect: '<ul><li>p</li><li>q</li></ul>' }],
  },
  {
    file: 'ok-destructured-props.vue',
    wrapper: "{ render() { return Vue.h(comp, { msg: 'hey' }) } }",
    steps: [{ run: html, expect: '<p>hey 0</p>' }],
  },
  {
    file: 'ok-expose-ref.vue',
    wrapper: '{ render() { return Vue.h(comp, { ref: (r) => { window.__vnRef = r } }) } }',
    steps: [
      { run: html, expect: '<div><p>none</p></div>' },
      { run: "window.__vnRef.hello('bob') || window.__vn.tick()", expect: '<div><p>hello bob</p></div>' },
    ],
  },
  {
    file: 'ok-provide-inject.vue',
    wrapper: "{ provide: { theme: 'dark' }, render() { return Vue.h(comp) } }",
    steps: [{ run: html, expect: '<p>dark</p>' }],
  },
  {
    file: 'ok-watch-lifecycle.vue',
    wrapper: H,
    steps: [
      { run: tick, expect: '<div><button>0</button><p>mounted</p></div>' },
      { run: "window.__vn.click('button')", expect: '<div><button>1</button><p>changed to 1</p></div>' },
    ],
  },
  {
    file: 'ok-scoped-style.vue',
    id: 'fx-ok-scoped-style',
    wrapper: H,
    steps: [
      { run: html, expect: '<p data-v-fx-ok-scoped-style="" class="s">scoped</p>' },
      { run: color('p'), expect: 'rgb(255, 0, 0)' },
    ],
  },
  {
    file: 'ok-template-only.vue',
    wrapper: H,
    steps: [{ run: html, expect: '<section class="static"><h1>Title</h1><p>Body text</p></section>' }],
  },
  {
    file: 'ok-normal-plus-setup.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<button>1</button>' },
      { run: "window.__vn.click('button')", expect: '<button>2</button>' },
    ],
  },
  {
    file: 'ok-v-bind-css.vue',
    wrapper: H,
    steps: [
      { run: `${tick}.then(() => ${color('p.c')})`, expect: 'rgb(255, 0, 0)' },
      { run: `window.__vn.click('p.c').then(() => ${color('p.c')})`, expect: 'rgb(0, 0, 255)' },
    ],
  },
  {
    file: 'ok-normal-template.vue',
    wrapper: H,
    steps: [{ run: html, expect: '<div class="plain">static</div>' }],
  },
  {
    file: 'ok-interpolation-escape.vue',
    wrapper: H,
    steps: [{ run: html, expect: '<p>&lt;b&gt;bold&lt;/b&gt; &amp; "q"</p>' }],
  },
  {
    file: 'ok-computed.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<div><button>2</button><span>4</span></div>' },
      { run: "window.__vn.click('button')", expect: '<div><button>3</button><span>6</span></div>' },
    ],
  },
  {
    file: 'ok-v-show.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<div><button>toggle</button><p>shown</p></div>' },
      { run: "window.__vn.click('button')", expect: '<div><button>toggle</button><p style="display: none;">shown</p></div>' },
    ],
  },
  {
    file: 'ok-event-modifiers.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<div><button>0</button></div>' },
      { run: "window.__vn.click('button')", expect: '<div><button>10</button></div>' },
    ],
  },
  {
    file: 'ok-dynamic-component.vue',
    wrapper: H,
    steps: [
      { run: html, expect: '<div><button>swap</button>first</div>' },
      { run: "window.__vn.click('button')", expect: '<div><button>swap</button>second</div>' },
    ],
  },
  {
    file: 'ok-custom-directive.vue',
    wrapper: H,
    steps: [{ run: html, expect: '<p data-fill="blue">x</p>' }],
  },
  {
    file: 'ok-ts-template.vue',
    wrapper: H,
    steps: [{ run: html, expect: '<p title="2">3</p>' }],
  },
  {
    file: 'ok-binding-named-like-global.vue',
    wrapper: H,
    steps: [{ run: html, expect: '<p>here proxy</p>' }],
  },
  {
    file: 'ok-src-template.vue',
    wrapper: H,
    steps: [{ run: html, expect: '<p class="part">from partial</p>' }],
  },
  {
    file: 'ok-style-src.vue',
    id: 'fx-ok-style-src',
    wrapper: H,
    steps: [
      { run: html, expect: '<p data-v-fx-ok-style-src="" class="q">q</p>' },
      { run: color('p'), expect: 'rgb(0, 128, 0)' },
    ],
  },
];

function compileCase(c) {
  const src = fs.readFileSync(path.join(FIXTURES, c.file), 'utf8');
  const id = c.id ?? `fx-${c.file.replace(/\.vue$/, '')}`;
  const r = compileSfc(src, { filename: c.file, rootDir: FIXTURES, id });
  return { r, id };
}

describe('real-Vue execution oracle (headless Chrome, vue@3.5.43)', () => {
  let page;
  before(async () => {
    if (skip) return;
    page = await openPage();
  });
  after(async () => {
    if (skip) return;
    await page?.close();
  });

  for (const c of CASES) {
    test(`${c.file} renders and reacts as Vue does`, skipOpt, async () => {
      const { r } = compileCase(c);
      assert.equal(r.diagnostics.filter((d) => d.severity === 'error').length, 0, 'fixture compiles without errors');
      const css = r.styles.map((s) => s.code).join('\n');
      await page.setContent(pageFor(r.code, r.lang, c.wrapper, css));
      for (const step of c.steps) {
        assert.equal(await page.evaluate(step.run), step.expect, `step: ${step.run}`);
      }
      assert.deepEqual(page.exceptions, [], 'no uncaught exceptions in the page');
    });
  }

  // Negative control: a wrong compiled output must be caught by the same expectation.
  test('negative control: a mutated compiled counter is detected', skipOpt, async () => {
    const { r } = compileCase({ file: 'ok-counter.vue' });
    const mutated = r.code.replace('count.value++', 'count.value--');
    assert.notEqual(mutated, r.code, 'the mutation changed the generated code');
    await page.setContent(pageFor(mutated, r.lang, H, r.styles.map((s) => s.code).join('\n')));
    assert.equal(await page.evaluate(html), '<button type="button">0</button>');
    assert.notEqual(await page.evaluate("window.__vn.click('button')"), '<button type="button">1</button>');
  });

  test('every Chrome process started by the oracle is closed', skipOpt, async () => {
    const pid = page.pid;
    await page.close();
    page = await openPage();
    assert.equal(isAlive(pid), false, `browser pid ${pid} is gone after close`);
  });
});
