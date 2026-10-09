// Native modules (WP-C1c): bare specifiers the runtime registers instead of compiling. `vue` and `@vue-native/platform`
// (the platform module of the manifest, row L19). Their exports are bound through module_import_cell; the generated
// code never contains their implementation. Every other bare specifier is outside the closed world.
export const NATIVE_MODULES = Object.freeze(['vue', '@vue-native/platform']);

export function isNativeSpecifier(specifier) {
  return NATIVE_MODULES.includes(specifier);
}
