// Node module hooks for the project oracle (WP-C1c): native specifiers of a project (native.json) resolve to generated
// JavaScript modules with the same exports as the Rust natives (tooling/harness/natives.mjs). Everything else is Node's.
import { nativeEntries, nativeModuleJs } from './natives.mjs';

let entries = [];
let specs = new Set();

export function initialize(data) {
  entries = nativeEntries(data?.natives ?? {});
  specs = new Set(entries.map((e) => e.specifier));
}

export async function resolve(specifier, context, nextResolve) {
  if (specs.has(specifier)) {
    return { url: `vn-native:${encodeURIComponent(specifier)}`, shortCircuit: true };
  }
  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  if (url.startsWith('vn-native:')) {
    const specifier = decodeURIComponent(url.slice('vn-native:'.length));
    return { format: 'module', source: nativeModuleJs(specifier, entries), shortCircuit: true };
  }
  return nextLoad(url, context);
}
