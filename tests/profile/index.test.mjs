import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isSupported, loadManifest, lookup } from '../../compiler/profile/index.mjs';

// Expected values are hand-checked against design.md section 8 and the maintainer's decisions, not read back from
// the manifest.

test('the manifest loads with schemaVersion 1, vue 3.5.43, default deny and the 126 design rows', () => {
  const manifest = loadManifest();
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.vueReference, '3.5.43');
  assert.equal(manifest.default, 'deny');
  assert.equal(manifest.rows.length, 126);
  assert.equal(Object.isFrozen(manifest), true);
});

test('lookup returns { status, row, note, compilerInternal } for named entries and null for unknown names', () => {
  assert.deepEqual(lookup('vue', 'ref'), { status: 'P', row: 'V01', note: null, compilerInternal: false });
  assert.deepEqual(lookup('vue', 'useTemplateRef'), { status: 'A', row: 'V27', note: null, compilerInternal: false });
  assert.deepEqual(lookup('macro', 'defineProps'), { status: 'P', row: 'S06', note: null, compilerInternal: false });
  assert.deepEqual(lookup('syntax', 'with'), { status: 'X', row: 'L01', note: 'Rejected by L01.', compilerInternal: false });
  assert.deepEqual(lookup('module', 'require'), { status: 'X', row: 'L16', note: 'CommonJS is excluded (L16).', compilerInternal: false });
  assert.deepEqual(lookup('builtin', 'eval'), { status: 'X', row: 'L20', note: null, compilerInternal: false });
  assert.deepEqual(lookup('app', 'app.mount'), { status: 'P', row: 'A01', note: null, compilerInternal: false });
  assert.deepEqual(lookup('instance', '$watch'), { status: 'X', row: 'A18', note: null, compilerInternal: false });
  assert.equal(lookup('vue', 'definitelyNotAnExport'), null);
  assert.equal(lookup('no-such-kind', 'ref'), null);
  assert.equal(lookup('vue', 42), null);
});

test('prototype property names do not resolve to inherited values', () => {
  for (const name of ['constructor', '__proto__', 'toString', 'hasOwnProperty']) {
    assert.equal(lookup('vue', name), null, name);
    assert.equal(isSupported('vue', name, 'web'), false, name);
    assert.equal(isSupported('vue', name, 'web', { origin: 'compiler' }), false, name);
  }
});

test('P and A names are supported on both targets', () => {
  assert.equal(isSupported('vue', 'ref', 'native'), true);
  assert.equal(isSupported('vue', 'ref', 'web'), true);
  assert.equal(isSupported('vue', 'useTemplateRef', 'native'), true);
  assert.equal(isSupported('vue', 'Teleport', 'web'), true);
  assert.equal(isSupported('directive', 'v-model (checkbox)', 'native'), true);
});

test('W names are supported only on the web target', () => {
  assert.equal(isSupported('vue', 'createSSRApp', 'web'), true);
  assert.equal(isSupported('vue', 'createSSRApp', 'native'), false);
  assert.equal(isSupported('modifier', '.prop', 'web'), true);
  assert.equal(isSupported('modifier', '.prop', 'native'), false);
});

test('X, E and unknown names are never supported for user code', () => {
  for (const target of ['native', 'web']) {
    assert.equal(isSupported('vue', 'createRenderer', target), false, 'X vue export');
    assert.equal(isSupported('vue', 'ReactiveEffect', target), false, 'X vue class');
    assert.equal(isSupported('vue', 'resolveComponent', target), false, 'X vue render helper');
    assert.equal(isSupported('directive', 'v-html', target), false, 'X directive');
    assert.equal(isSupported('vue', 'Suspense', target), false, 'E component');
    assert.equal(isSupported('syntax', 'top-level await', target), false, 'E syntax');
    assert.equal(isSupported('vue', 'definitelyNotAnExport', target), false, 'unknown');
  }
});

test('an unknown target or origin is an error, not a guess', () => {
  assert.throws(() => isSupported('vue', 'ref'), TypeError);
  assert.throws(() => isSupported('vue', 'ref', 'desktop'), TypeError);
  assert.throws(() => isSupported('vue', 'ref', 'web', { origin: 'desktop' }), TypeError);
  assert.throws(() => isSupported('vue', 'ref', 'web', null), TypeError);
});

test('lookup returns a copy, so callers cannot change later answers', () => {
  const first = lookup('vue', 'ref');
  first.status = 'X';
  assert.deepEqual(lookup('vue', 'ref'), { status: 'P', row: 'V01', note: null, compilerInternal: false });
});

test('origin user (default): compilerInternal helpers are not supported on either target', () => {
  for (const target of ['native', 'web']) {
    assert.deepEqual(lookup('vue', 'openBlock'), { status: 'X', row: 'A11', note: null, compilerInternal: true });
    assert.equal(isSupported('vue', 'openBlock', target), false, target);
    assert.equal(isSupported('vue', 'openBlock', target, { origin: 'user' }), false, target);
    assert.equal(isSupported('vue', 'vModelText', target), false, target);
  }
});

test('origin compiler: compilerInternal helpers are supported on both targets', () => {
  for (const target of ['native', 'web']) {
    assert.equal(isSupported('vue', 'openBlock', target, { origin: 'compiler' }), true, target);
    assert.equal(isSupported('vue', 'withCtx', target, { origin: 'compiler' }), true, target);
    assert.equal(isSupported('vue', 'mergeDefaults', target, { origin: 'compiler' }), true, target);
  }
});

test('origin compiler does not grant names the official compiler never emits', () => {
  for (const name of ['assertNumber', 'pushScopeId', 'popScopeId', 'capitalize', 'resolveFilter', 'BaseTransition', 'Suspense']) {
    for (const target of ['native', 'web']) {
      assert.equal(isSupported('vue', name, target, { origin: 'compiler' }), false, `${name} ${target}`);
    }
  }
});

test('the promoted render helpers, instance accessor and effect API are A or P: supported for user code on both targets', () => {
  const promoted = {
    h: 'A', isVNode: 'A', cloneVNode: 'A', mergeProps: 'P', getCurrentInstance: 'A',
    effect: 'P', stop: 'P', proxyRefs: 'P', getCurrentWatcher: 'P',
  };
  for (const [name, status] of Object.entries(promoted)) {
    assert.equal(lookup('vue', name).status, status, `${name} status`);
    assert.equal(lookup('vue', name).compilerInternal, false, `${name} is not compilerInternal`);
    for (const target of ['native', 'web']) {
      assert.equal(isSupported('vue', name, target), true, `${name} ${target}`);
    }
  }
  assert.equal(lookup('vue', 'h').row, 'A11');
  assert.equal(lookup('vue', 'getCurrentInstance').row, 'A18');
  assert.equal(lookup('vue', 'mergeProps').row, 'U13');
  assert.equal(lookup('vue', 'proxyRefs').row, 'V07');
  assert.equal(lookup('vue', 'getCurrentWatcher').row, 'V16');
  assert.equal(lookup('vue', 'effect').row, 'V12');
  assert.equal(lookup('vue', 'stop').row, 'V12');
});

test('the names kept X in the same families stay unsupported: ReactiveEffect, resolveDirective, withDirectives', () => {
  for (const name of ['ReactiveEffect', 'resolveDirective', 'withDirectives', 'withModifiers', 'createRenderer']) {
    for (const target of ['native', 'web']) {
      assert.equal(lookup('vue', name).status, 'X', name);
      assert.equal(isSupported('vue', name, target), false, `${name} ${target}`);
    }
  }
});

test('origin does not change public entries: P stays supported for both origins, E stays unsupported', () => {
  assert.equal(isSupported('vue', 'unref', 'web', { origin: 'compiler' }), true);
  assert.equal(isSupported('vue', 'unref', 'web', { origin: 'user' }), true);
  assert.equal(isSupported('vue', 'withAsyncContext', 'web', { origin: 'compiler' }), false);
});

test('CSS contract entries are supported (design 9.2: useCssVars and useCssModule)', () => {
  assert.equal(lookup('vue', 'useCssVars').status, 'P');
  assert.equal(lookup('vue', 'useCssVars').row, 'U03');
  assert.equal(isSupported('vue', 'useCssVars', 'native'), true);
  assert.equal(isSupported('vue', 'useCssModule', 'web'), true);
});

test('native tags: supported tags are A, unlisted and custom-element names are unsupported', () => {
  assert.deepEqual(lookup('native-tag', 'div'), { status: 'A', row: 'U31', note: null, compilerInternal: false });
  assert.equal(isSupported('native-tag', 'div', 'native'), true);
  assert.equal(isSupported('native-tag', 'textarea', 'web'), true);
  assert.equal(lookup('native-tag', 'definitelyNotATag'), null);
  assert.equal(isSupported('native-tag', 'my-element', 'web'), false);
});

test('native tags: excluded tags are X with a use-instead note', () => {
  const table = lookup('native-tag', 'table');
  assert.equal(table.status, 'X');
  assert.match(table.note, /framework grid component instead/);
  assert.equal(lookup('native-tag', 'td').status, 'X');
  assert.match(lookup('native-tag', 'svg').note, /image asset \(img\) instead/);
  assert.match(lookup('native-tag', 'video').note, /web target instead/);
  assert.match(lookup('native-tag', 'iframe').note, /link \(a\) instead/);
  assert.equal(isSupported('native-tag', 'video', 'web'), false);
});

test('special template tags stay under their own rows and are not native tags', () => {
  assert.equal(lookup('native-tag', 'template'), null);
  assert.equal(lookup('native-tag', 'slot'), null);
  assert.deepEqual(lookup('element', 'template'), { status: 'P', row: 'U30', note: 'Template fragment element (U30). The SFC <template> block is S01.', compilerInternal: false });
  assert.equal(lookup('sfc', '<template>').row, 'S01');
  assert.equal(lookup('element', 'slot').row, 'U12');
  assert.equal(lookup('component', 'component').row, 'U19');
  assert.equal(lookup('component', 'transition').row, 'U23');
  assert.equal(lookup('component', 'keep-alive').row, 'U21');
  assert.equal(lookup('component', 'suspense').status, 'E');
  assert.equal(isSupported('component', 'suspense', 'web'), false);
});
