// WP-RT-C component goldens. The scenarios below are plain JavaScript run in headless Chrome against vue@3.5.43 (production
// global build). Each scenario mounts its root component with the props of each step (null = unmount) into a container and
// records innerHTML after every step. The same scenarios are written in Rust in crates/runtime/tests/rtc_components.rs.
// Usage (repository root): node tests/ui/runtime-c/record-components.mjs [--check]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { withPage } from '../../../tooling/chrome/chrome.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const outDir = path.join(here, 'components');
const check = process.argv.includes('--check');

// Runs in the page. Returns { scenarioName: [innerHTML after each step] }.
function runComponents() {
  const V = window.Vue;
  const { h } = V;
  const out = {};
  const run = (name, makeRoot, steps) => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const root = makeRoot();
    const snaps = [];
    for (const props of steps) {
      if (props === null) V.render(null, container);
      else V.render(h(root, props), container);
      snaps.push(container.innerHTML);
    }
    out[name] = snaps;
    container.remove();
  };

  run('setup-render-fn', () => ({
    setup() {
      return () => h('p', { class: 'c' }, 'hi');
    },
  }), [{}, null]);

  run('props-attrs-fallthrough', () => {
    const Inner = {
      props: ['msg', 'count'],
      render() {
        return h('span', { class: 'inner' }, this.msg + ':' + this.count);
      },
    };
    return {
      props: ['msg', 'n'],
      render() {
        return h(Inner, { msg: this.msg, count: this.n, class: 'outer', id: 'x' });
      },
    };
  }, [{ msg: 'a', n: 1 }, { msg: 'b', n: 1 }, { msg: 'b', n: 2 }]);

  run('props-casting-defaults', () => {
    const Typed = {
      props: {
        flag: Boolean,
        'first-name': { type: String, default: 'anon' },
        items: { type: Array, default: () => [1, 2] },
        n: { type: Number, default: 5 },
      },
      render() {
        return h('i', null, [String(this.flag), this.firstName, String(this.items.length), String(this.n)].join('|'));
      },
    };
    return {
      props: ['attrsFlag', 'fn', 'n'],
      render() {
        return h(Typed, { flag: this.attrsFlag, 'first-name': this.fn, n: this.n });
      },
    };
  }, [
    { attrsFlag: true, fn: 'Bob', n: 3 },
    { attrsFlag: '', fn: undefined, n: undefined },
    { attrsFlag: undefined, fn: 'Al', n: 'x' },
  ]);

  run('slots-default-named', () => {
    const Card = {
      render() {
        return h('div', { class: 'card' }, [
          h('h2', null, this.$slots.header ? this.$slots.header() : 'none'),
          h('section', null, this.$slots.default ? this.$slots.default() : 'empty'),
        ]);
      },
    };
    return {
      props: ['title'],
      render() {
        return h(Card, null, {
          header: () => this.title,
          default: () => [h('b', null, 'body'), ' tail'],
        });
      },
    };
  }, [{ title: 'T1' }, { title: 'T2' }]);

  run('emit-and-lifecycle-order', () => {
    const log = [];
    const Child = {
      props: ['v'],
      emits: ['ping'],
      setup(props, { emit }) {
        V.onBeforeMount(() => log.push('bm'));
        V.onMounted(() => {
          log.push('m');
          emit('ping', props.v);
        });
        V.onBeforeUpdate(() => log.push('bu'));
        V.onUpdated(() => log.push('u'));
        V.onUnmounted(() => log.push('um'));
        return () => h('em', null, 'v=' + props.v);
      },
    };
    return {
      props: ['v', 'show'],
      render() {
        return h('div', null, [
          h('p', null, log.join(',')),
          this.show ? h(Child, { v: this.v, onPing: (x) => log.push('ping:' + x) }) : null,
        ]);
      },
    };
  }, [{ v: 1, show: true }, { v: 2, show: true }, { v: 2, show: false }]);

  run('render-error-comment', () => ({
    props: ['bad'],
    render() {
      if (this.bad) throw new Error('boom');
      return h('p', null, 'ok');
    },
  }), [{ bad: false }, { bad: true }, { bad: false }]);

  run('keyed-component-list', () => {
    const Item = {
      props: ['id'],
      render() {
        return h('li', null, 'item' + this.id);
      },
    };
    return {
      props: ['ids'],
      render() {
        return h('ul', null, this.ids.map((id) => h(Item, { key: id, id })));
      },
    };
  }, [{ ids: [1, 2, 3] }, { ids: [3, 1, 2] }, { ids: [2] }, { ids: [4, 2, 5] }]);

  run('inherit-attrs-false', () => {
    const Child = {
      inheritAttrs: false,
      render() {
        return h('span', { title: this.$attrs.title }, 'x');
      },
    };
    return {
      props: ['t'],
      render() {
        return h(Child, { title: this.t, class: 'k' });
      },
    };
  }, [{ t: 'a' }, { t: 'b' }]);

  return out;
}

const vueSrc = fs.readFileSync(path.join(root, 'node_modules/vue/dist/vue.global.prod.js'), 'utf8');
const html = `<!doctype html><meta charset="utf-8"><body><script>${vueSrc}</script></body>`;
const golden = await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
  await page.setContent(html);
  const version = await page.evaluate(() => window.Vue.version);
  if (version !== '3.5.43') throw new Error(`expected vue 3.5.43, got ${version}`);
  return page.evaluate(runComponents);
});

fs.mkdirSync(outDir, { recursive: true });
const goldenPath = path.join(outDir, 'golden.json');
const goldenJson = JSON.stringify(golden, null, 1) + '\n';
if (check) {
  const same = fs.existsSync(goldenPath) && fs.readFileSync(goldenPath, 'utf8') === goldenJson;
  console.log(`golden ${same ? 'matches' : 'DIFFERS'} (${Object.keys(golden).length} scenarios)`);
  process.exitCode = same ? 0 : 1;
} else {
  fs.writeFileSync(goldenPath, goldenJson);
  console.log(`wrote ${Object.keys(golden).length} component scenarios to ${path.relative(root, outDir)}`);
}
