// SFC frontend diagnostic codes (WP-F1). Registered here, not in compiler/diagnostics/codes.mjs (owned by WP-C1a).
// Ranges: VN60xx blocks, scripts and styles; VN61xx template; VN62xx imports from vue and node built-ins.
// Profile and scope findings keep their VN1xxx-VN5xxx codes from the frontend registry.
import { createDiagnostic } from '../diagnostics/diagnostic.mjs';
import { CODES as FRONTEND_CODES } from '../diagnostics/codes.mjs';

function entry(title, explanation, hint) {
  return Object.freeze({ title, explanation, hint });
}

export const SFC_CODES = Object.freeze({
  VN6001: entry(
    'SFC syntax error',
    'The .vue file is not a valid single-file component. The block parser reported the error at the span.',
    'Fix the markup at the reported span.',
  ),
  VN6002: entry(
    'Duplicate block',
    'A component may contain at most one <template>, one <script> and one <script setup> block.',
    'Remove the extra block or merge its content into the first one.',
  ),
  VN6003: entry(
    'Custom block',
    'Custom blocks such as <docs> or <i18n> need a transform, and no transform is installed (S04).',
    'Move the content into a <script> block or into a module that the script imports.',
  ),
  VN6004: entry(
    'Preprocessor language',
    'Preprocessor languages (scss, sass, less, stylus, postcss, pug) need transforms that are not installed (S04).',
    'Write plain CSS or HTML, or remove the lang attribute.',
  ),
  VN6005: entry(
    'External script block',
    '<script src> and <script setup src> are rejected: the script must be part of the component file (S03).',
    'Move the code into the block, or import it from a module.',
  ),
  VN6006: entry(
    'Unsupported script language',
    'Only lang="js" and lang="ts" are supported for <script> and <script setup>. JSX and TSX are excluded (S21).',
    'Use lang="ts" for typed code, or plain JavaScript.',
  ),
  VN6007: entry(
    'Options API',
    'Options API component definitions are excluded. The default export of a normal <script> is an object literal with name or inheritAttrs, or defineComponent of an object literal (which may also have setup, A10) or of one setup function. Other entries, such as render or data, are Options API (S02, A18).',
    'Use <script setup> with defineProps, defineEmits, ref and computed instead.',
  ),
  VN6008: entry(
    'Block file unreadable or outside the project',
    'A template or style src file cannot be read, or its real path leaves the project root (S03).',
    'Point src at a file inside the project root.',
  ),
  VN6009: entry(
    'Export in <script setup>',
    '<script setup> cannot contain ES module exports (S01).',
    'Move the export into a normal <script> block or into a module.',
  ),
  VN6010: entry(
    'Component without template or script',
    'The file has neither a <template> nor a script block, so it would render nothing.',
    'Add a <template> block.',
  ),
  VN6011: entry(
    'Reserved name',
    'The generated component uses the names __sfc, and render when a template is present. User bindings may not use them.',
    'Rename the binding.',
  ),
  VN6012: entry(
    'Script compile error',
    'The official compiler (compileScript from @vue/compiler-sfc 3.5.43) rejected a <script setup> or <script> block. Its errors carry no location, so the span is the block tag.',
    'Fix the macro or script usage named in the message.',
  ),
  VN6013: entry(
    'Template compile error',
    'The official compiler (compileTemplate from @vue/compiler-sfc 3.5.43) rejected the template.',
    'Fix the template at the reported span.',
  ),
  VN6014: entry(
    'Style compile error',
    'The official compiler (compileStyle from @vue/compiler-sfc 3.5.43) rejected a style block.',
    'Fix the CSS at the reported span.',
  ),
  VN6015: entry(
    'CSS modules',
    '<style module> needs asynchronous CSS module compilation. This synchronous frontend does not provide it (S03).',
    'Use a scoped style block, or give the elements class names in the template.',
  ),
  VN6016: entry(
    'Redundant or deprecated form',
    'A warning, not an error: the input compiles, but the official compiler or the profile check reports a form that is redundant or that Vue prints a warning for.',
    'Read the message and remove the redundant form if you can.',
  ),
  VN6101: entry(
    'Unknown tag or component',
    'The tag is not a native tag of the profile (U31), not a component imported or declared in <script setup>, and not a supported built-in. Custom elements are excluded (U19, U32).',
    'Use one of the native tags, or import the component in <script setup>.',
  ),
  VN6102: entry(
    'Excluded native tag',
    'The tag exists in HTML but is excluded from the native profile (U31, U32).',
    'Use the alternative named in the message.',
  ),
  VN6103: entry(
    'Experimental built-in component',
    '<Suspense> is experimental (E) and not promised for production (U25).',
    'Use a conditional render instead.',
  ),
  VN6104: entry(
    'Excluded built-in component',
    'BaseTransition is excluded. Only Transition is promised (U23).',
    'Use <Transition>.',
  ),
  VN6110: entry(
    'v-html',
    'v-html inserts raw markup. It is excluded from the native profile (U29). The web target allows it only with the trustedHtml option.',
    'Render text with interpolation, or build for the web target with trustedHtml.',
  ),
  VN6111: entry(
    'The .passive modifier',
    'The .passive modifier is excluded from the profile (U06).',
    'Remove it. Call event.preventDefault() where a listener must not be passive.',
  ),
  VN6112: entry(
    'Unsupported input type',
    'Only text, checkbox, radio and range inputs are in the native profile (U10). file and date use explicit controls.',
    'Use the explicit control named in the message.',
  ),
  VN6113: entry(
    'Dynamic input type',
    'A dynamic input type cannot be checked at compile time, so it is rejected (U10).',
    'Use a static type attribute.',
  ),
  VN6114: entry(
    'Unknown or unbound directive',
    'A custom directive must be bound in <script setup> under the name v<Name> (U26). The directive is not bound here.',
    'Declare the directive in <script setup>, or remove it.',
  ),
  VN6115: entry(
    'Unsupported directive modifier',
    'The modifier is unknown to the profile, is excluded, or is web-only on the native target (U04, U06, U07, U08).',
    'Remove the modifier, or build for the web target.',
  ),
  VN6116: entry(
    'Excluded attribute',
    'The attribute is excluded from the native profile, or is web-only (U32, U04).',
    'Remove the attribute.',
  ),
  VN6117: entry(
    'Instance property not supported',
    'The public instance property is excluded from the profile (A18).',
    'Pass the value explicitly through props, emits or a binding.',
  ),
  VN6201: entry(
    'Excluded export of vue',
    'This vue export is not part of the profile. The compatibility manifest gives the reason (docs/compatibility).',
    'Use the supported API named in the manifest note.',
  ),
  VN6202: entry(
    'Default or namespace import from vue',
    'The profile has named imports from vue only. A namespace or default import would expose excluded exports.',
    'Import the names you use, one by one.',
  ),
  VN6203: entry(
    'Import from a vue subpath',
    'vue/* subpaths such as vue/server-renderer are not part of the native profile (A15).',
    'Use the platform renderer API instead.',
  ),
  VN6204: entry(
    'Node built-in module',
    'Node built-in modules are not available in the profile (L16).',
    'Use the @vue-native platform services.',
  ),
});

export function isSfcCode(code) {
  return typeof code === 'string' && Object.hasOwn(SFC_CODES, code);
}

export function sfcCodeInfo(code) {
  if (!isSfcCode(code)) throw new RangeError(`unknown SFC diagnostic code: ${String(code)}`);
  return SFC_CODES[code];
}

// Diagnostic for a VN6xxx code or a frontend code. The frontend Diagnostic class validates codes against the
// frontend registry only, so an SFC code is built on a registered placeholder and then given its own code. Positions
// and message checks are still done by the Diagnostic constructor.
export function createAnyDiagnostic(code, { message, hint, severity = 'error', file, start, end }) {
  if (!isSfcCode(code)) {
    if (!Object.hasOwn(FRONTEND_CODES, code)) throw new RangeError(`unknown diagnostic code: ${String(code)}`);
    return createDiagnostic(code, { message, hint, severity, file, start, end });
  }
  const info = sfcCodeInfo(code);
  const diag = createDiagnostic('VN9001', {
    message: message ?? info.title,
    hint: hint ?? info.hint,
    severity,
    file,
    start,
    end,
  });
  diag.code = code;
  return diag;
}
