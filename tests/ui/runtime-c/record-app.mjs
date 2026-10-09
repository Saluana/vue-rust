// WP-RT-C app, ref and directive goldens: plain JavaScript run in headless Chrome against vue@3.5.43 (production global
// build). Each scenario records the container's outerHTML after each step and a log of the callbacks in call order.
// The same scenarios are written in Rust in crates/runtime/tests/rtc_app.rs. Usage: node tests/ui/runtime-c/record-app.mjs [--check]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { withPage } from '../../../tooling/chrome/chrome.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const outDir = path.join(here, 'app');
const check = process.argv.includes('--check');

// Runs in the page. Returns { scenarioName: { snaps: [outerHTML...], log: [...] } }.
async function runApp() {
  const V = window.Vue;
  const { h, createApp, withDirectives } = V;
  const out = {};
  const container = () => {
    const d = document.createElement('div');
    d.id = 'app';
    document.body.appendChild(d);
    return d;
  };

  {
    const c = container();
    const app = createApp({ render() { return h('p', null, 'hello'); } });
    const snaps = [];
    app.mount('#app');
    snaps.push(c.outerHTML);
    app.unmount();
    snaps.push(c.outerHTML);
    out['app-mount-unmount'] = { snaps, log: [] };
    c.remove();
  }

  {
    const c = container();
    const log = [];
    const Root = {
      props: ['v'],
      render() {
        return h('p', { ref: (el) => log.push(el ? 'el:' + el.tagName : 'null') }, 'x' + this.v);
      },
    };
    const app = createApp(Root, { v: 7 });
    const snaps = [];
    app.mount('#app');
    snaps.push(c.outerHTML);
    app.unmount();
    snaps.push(c.outerHTML);
    out['function-ref-order'] = { snaps, log: log.slice() };
    c.remove();
  }

  {
    const c = container();
    const log = [];
    const dir = {
      created() { log.push('created'); },
      beforeMount() { log.push('beforeMount'); },
      mounted(el, b) {
        el.setAttribute('data-m', String(b.value));
        log.push('mounted:' + b.value);
      },
      beforeUnmount() { log.push('beforeUnmount'); },
      unmounted() { log.push('unmounted'); },
    };
    const app = createApp({
      render() {
        return withDirectives(h('p', null, 'x'), [[dir, 42]]);
      },
    });
    const snaps = [];
    app.mount('#app');
    snaps.push(c.outerHTML);
    app.unmount();
    snaps.push(c.outerHTML);
    out['directive-hooks'] = { snaps, log: log.slice() };
    c.remove();
  }
  {
    const c = container();
    let count;
    const Root = {
      setup() {
        count = V.ref(0);
        return () => h('b', null, String(count.value));
      },
    };
    const app = createApp(Root);
    const snaps = [];
    app.mount('#app');
    snaps.push(c.outerHTML);
    count.value = 5;
    await Promise.resolve();
    await Promise.resolve();
    snaps.push(c.outerHTML);
    count.value = 9;
    await Promise.resolve();
    await Promise.resolve();
    snaps.push(c.outerHTML);
    app.unmount();
    snaps.push(c.outerHTML);
    out['reactive-ref-update'] = { snaps, log: [] };
    c.remove();
  }
  {
    const c = container();
    const Child = { __scopeId: 'data-v-child', render() { return h('p', null, 'c'); } };
    const Card = {
      __scopeId: 'data-v-card',
      render() {
        return h('section', null, [V.renderSlot(this.$slots, 'default')]);
      },
    };
    const Root = {
      __scopeId: 'data-v-root',
      render() {
        return h('div', null, [
          h('span', null, 's'),
          h(Child),
          h(Card, null, { default: () => h('b', null, 'x') }),
        ]);
      },
    };
    const app = createApp(Root);
    const snaps = [];
    app.mount('#app');
    snaps.push(c.outerHTML);
    app.unmount();
    snaps.push(c.outerHTML);
    out['scoped-styles'] = { snaps, log: [] };
    c.remove();
  }
  {
    const c = container();
    const Child = {
      props: ['modelValue'],
      emits: ['update:modelValue'],
      setup(props) {
        const model = V.useModel(props, 'modelValue');
        V.onMounted(() => {
          model.value = 'child';
        });
        return () => h('i', null, 'c:' + model.value);
      },
    };
    const Root = {
      setup() {
        const v = V.ref('start');
        return () =>
          h('div', null, [
            h('b', null, 'p:' + v.value),
            h(Child, { modelValue: v.value, 'onUpdate:modelValue': (x) => { v.value = x; } }),
          ]);
      },
    };
    const app = createApp(Root);
    const snaps = [];
    app.mount('#app');
    snaps.push(c.outerHTML);
    await Promise.resolve();
    await Promise.resolve();
    snaps.push(c.outerHTML);
    app.unmount();
    snaps.push(c.outerHTML);
    out['use-model-emit'] = { snaps, log: [] };
    c.remove();
  }

  {
    const c = container();
    const MD = { props: V.mergeDefaults(['label'], { label: 'dflt' }), render() { return h('i', null, this.label); } };
    const MM = { props: V.mergeModels(['a'], ['b']), render() { return h('u', null, this.a + this.b); } };
    const RP = {
      props: ['a', 'b', 'c'],
      setup(props) {
        const rest = V.createPropsRestProxy(props, ['a']);
        return () => h('s', null, Object.keys(rest).join(','));
      },
    };
    const Root = { render() { return h('div', null, [h(MD), h(MM, { a: '1', b: '2' }), h(RP, { a: 'x', b: 'y', c: 'z' })]); } };
    const app = createApp(Root);
    const snaps = [];
    app.mount('#app');
    snaps.push(c.outerHTML);
    app.unmount();
    snaps.push(c.outerHTML);
    out['props-helpers'] = { snaps, log: [] };
    c.remove();
  }
  return out;
}

const vueSrc = fs.readFileSync(path.join(root, 'node_modules/vue/dist/vue.global.prod.js'), 'utf8');
const html = `<!doctype html><meta charset="utf-8"><body><script>${vueSrc}</script></body>`;
const golden = await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
  await page.setContent(html);
  const version = await page.evaluate(() => window.Vue.version);
  if (version !== '3.5.43') throw new Error(`expected vue 3.5.43, got ${version}`);
  return page.evaluate(runApp);
});

fs.mkdirSync(outDir, { recursive: true });
const goldenPath = path.join(outDir, 'golden.json');
const goldenJson = JSON.stringify(golden, null, 1) + '\n';
if (check) {
  const same = fs.existsSync(goldenPath) && fs.readFileSync(goldenPath, 'utf8') === goldenJson;
  console.log(`golden ${same ? 'matches' : 'DIFFERS'}`);
  process.exitCode = same ? 0 : 1;
} else {
  fs.writeFileSync(goldenPath, goldenJson);
  console.log(`wrote ${Object.keys(golden).length} app scenarios to ${path.relative(root, outDir)}`);
}
