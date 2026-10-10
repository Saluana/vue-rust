// Frozen diagnostic code registry for the frontend.
// Ranges: VN1 parse, VN2 unsupported JavaScript, VN3 unsupported TypeScript, VN4 unsupported or unknown
// API/global, VN5 scope errors, VN7 type checker findings (compiler/types, pinned TypeScript 6), VN9 internal compiler
// bugs. Each entry is { title, explanation, hint }. Rule ids and row references live in compiler/analysis/profile-rules.mjs.

function entry(title, explanation, hint) {
  return Object.freeze({ title, explanation, hint });
}

export const CODES = Object.freeze({
  VN1000: entry(
    'Syntax error',
    'The parser rejected the source. Input must be a valid ES module (or a TypeScript module when lang is ts).',
    'Fix the syntax at the reported span.',
  ),

  VN2001: entry(
    "'with' statement",
    "The 'with' statement changes name resolution at run time. It is invalid in modules and excluded (L01).",
    'Read the properties you need explicitly, or bind the object to a const and use its properties.',
  ),
  VN2002: entry(
    'Class declaration or expression',
    'Classes and prototype-based inheritance are outside the profile (L05).',
    'Use a plain object, a factory function or a composable function instead of a class.',
  ),
  VN2003: entry(
    'extends clause',
    'Class inheritance is not supported (L05).',
    'Compose behaviour with functions or plain objects instead of inheritance.',
  ),
  VN2004: entry(
    'super reference',
    'super belongs to the excluded class and prototype model (L05).',
    'Call the shared function directly instead of using super.',
  ),
  VN2005: entry(
    'Private class name',
    'Private names (#name) belong to the excluded class model (L05).',
    'Use a closure variable or an ordinary property instead.',
  ),
  VN2006: entry(
    'Static initialization block',
    'Static blocks are class features and are excluded (L05).',
    'Move the initialization into a module-level const or function.',
  ),
  VN2007: entry(
    '`new` of an unsupported constructor',
    '`new` is supported only for the built-in constructors listed in the profile and for imported bindings (L05).',
    'Use a factory function, or import the constructor from a supported module.',
  ),
  VN2008: entry(
    'Generator function',
    'Generator and async generator functions are outside the profile (L15).',
    'Use an async function, or return the values as an array.',
  ),
  VN2009: entry(
    'yield expression',
    'yield is valid only inside generators, which are excluded (L15).',
    'Return the values explicitly instead of yielding them.',
  ),
  VN2010: entry(
    'for await loop',
    'for await...of is an async iteration protocol outside the profile (L15).',
    'Await each value explicitly inside an ordinary for...of loop.',
  ),
  VN2011: entry(
    'Regular expression literal',
    'Regular expressions are outside the first production profile (L18).',
    'Use String methods such as indexOf, startsWith or split.',
  ),
  VN2012: entry(
    'BigInt literal',
    'BigInt is not part of the profile (L08).',
    'Use a Number with documented bounds.',
  ),
  VN2013: entry(
    'Tagged template',
    'Tagged templates are not part of the profile.',
    'Call a function with the string as an ordinary argument.',
  ),
  VN2014: entry(
    'debugger statement',
    'The debugger statement is not part of the profile.',
    'Remove the statement.',
  ),
  VN2015: entry(
    'arguments object',
    'The arguments object is not part of the profile.',
    'Use a rest parameter (...args).',
  ),
  VN2016: entry(
    '__proto__ in an object literal',
    'Setting a prototype through __proto__ is prototype manipulation and is excluded (L05).',
    'Use an ordinary property with a different key.',
  ),
  VN2017: entry(
    '__proto__ property access',
    'Reading or writing __proto__ is prototype manipulation and is excluded (L05).',
    'Use ordinary properties; prototype replacement is not supported.',
  ),
  VN2018: entry(
    'delete of an identifier',
    'Deleting a variable name is an error in strict module code and is not part of the profile (L04).',
    'Delete a property of an object (delete obj.key), or assign undefined.',
  ),
  VN2019: entry(
    'import() with a computed specifier',
    'import() is supported only with a literal module specifier (L17).',
    'Use a string literal, for example import("./module.js").',
  ),
  VN2020: entry(
    'import.meta',
    'import.meta is not part of the profile.',
    'Pass the configuration explicitly through props or a service.',
  ),
  VN2021: entry(
    'new.target',
    'new.target belongs to the excluded constructor model (L05).',
    'Pass an explicit parameter or use a separate function.',
  ),
  VN2022: entry(
    'Top-level await',
    'Top-level await is not part of the initial profile (L16). Experimental Suspense support is separate (S14).',
    'Move the await into an async function and call that function from setup.',
  ),
  VN2023: entry(
    'Function declaration in a statement position',
    'Annex B function declarations in labelled or unbraced statement bodies are not part of the profile (L01).',
    'Wrap the function declaration in a block, or bind a function expression to a const.',
  ),
  VN2024: entry(
    'Legacy octal literal',
    'Legacy octal numeric literals are not allowed in module code (L08).',
    'Write the value in decimal, or use the 0o prefix.',
  ),
  VN2025: entry(
    'Legacy octal escape',
    'Legacy octal escape sequences in strings are not allowed in module code (L09).',
    'Write the code unit with a \\u or \\x escape.',
  ),
  VN2026: entry(
    'Assignment to eval or arguments',
    'eval and arguments cannot be assigned in strict module code (L02, L20).',
    'Use a differently named variable.',
  ),
  VN2027: entry(
    'Binding named eval or arguments',
    'eval and arguments cannot be declared as bindings in strict module code (L02).',
    'Rename the binding.',
  ),
  VN2028: entry(
    'JSX in JavaScript',
    'JSX is not part of the language profile. SFC templates are the supported view syntax (S01).',
    'use a <template> in a .vue file',
  ),
  VN2029: entry(
    'Decorator',
    'Decorators are not part of the profile (S21).',
    'Use an explicit function call instead of a decorator.',
  ),
  VN2030: entry(
    'Auto-accessor class field',
    'Auto-accessor class fields are not part of the profile (S21).',
    'Use a ref or a computed value.',
  ),

  VN3001: entry(
    'TypeScript enum',
    'Enums emit runtime objects and are excluded in the initial profile (S20).',
    'Use a union of string literal types with a const object or plain constants.',
  ),
  VN3002: entry(
    'TypeScript namespace or module declaration',
    'Namespaces emit runtime code and are excluded (S20).',
    'Use an ES module (import and export) instead of a namespace.',
  ),
  VN3003: entry(
    'import = alias',
    'import-equals declarations emit CommonJS-style runtime code and are excluded (S25).',
    'Use import { name } from "module".',
  ),
  VN3004: entry(
    'export = assignment',
    'export assignment is a CommonJS runtime form and is excluded (S25).',
    'Use named or default exports.',
  ),
  VN3005: entry(
    'Decorator (TypeScript)',
    'Decorators are excluded (S21).',
    'Use explicit function calls instead of a decorator.',
  ),
  VN3006: entry(
    'Constructor parameter property',
    'Parameter properties emit runtime assignments and are excluded (S20).',
    'Declare the field and assign it explicitly.',
  ),
  VN3007: entry(
    'Auto-accessor class field (TypeScript)',
    'Auto-accessor class fields are excluded (S21).',
    'Use a ref or a computed value.',
  ),
  VN3008: entry(
    'Explicit resource management',
    'using and await using declarations need completion lowering that is not implemented (S24).',
    'Use try/finally with an explicit dispose call.',
  ),
  VN3009: entry(
    'JSX or TSX',
    'JSX and TSX are not part of the profile (S21).',
    'use a <template> in a .vue file',
  ),
  VN4001: entry(
    'eval',
    'Dynamic code evaluation is rejected (L20).',
    'Write the logic directly as code.',
  ),
  VN4002: entry(
    'Function constructor',
    'Function(...) builds code at run time and is rejected (L20).',
    'Write the function directly.',
  ),
  VN4003: entry(
    'Proxy',
    'A user-visible Proxy is excluded. Reactive proxies are internal to the runtime (L20).',
    'Use ref, reactive or computed from the Vue API.',
  ),
  VN4004: entry(
    'Reflect',
    'Reflect is excluded (L20).',
    'Use the direct operation, such as property access or Object.keys.',
  ),
  VN4005: entry(
    'BigInt',
    'BigInt is excluded (L08).',
    'Use a Number with documented bounds.',
  ),
  VN4006: entry(
    'RegExp',
    'Regular expressions are excluded from the first profile (L18).',
    'Use String methods or a host validation service.',
  ),
  VN4007: entry(
    'Weak collections and finalizers',
    'WeakMap, WeakSet, WeakRef and FinalizationRegistry are excluded initially (L11).',
    'Use Map or Set and remove entries explicitly.',
  ),
  VN4008: entry(
    'Property descriptor or prototype API',
    'Object.defineProperty, Object.defineProperties and Object.setPrototypeOf are metaprogramming and are excluded (L04, L05).',
    'Use plain object literals or explicit properties.',
  ),
  VN4009: entry(
    'CommonJS global',
    'require, module and exports are CommonJS and are excluded (L16).',
    'Use import and export.',
  ),
  VN4010: entry(
    'Node.js global',
    'Node.js globals are not available in the profile (S23).',
    'Use the @vue-native platform services.',
  ),
  VN4011: entry(
    'Browser DOM global in the native target',
    'Browser DOM globals are not available in the native profile (S23, R14).',
    'Use the @vue-native platform services, or build for the web target.',
  ),
  VN4012: entry(
    'Dynamic code through a constructor property',
    'Calling a constructor property can build code at run time and is rejected (L20).',
    'Write the logic directly.',
  ),
  VN4013: entry(
    'globalThis',
    'Access to the global object is excluded (L20).',
    'Import the value you need.',
  ),
  VN4014: entry(
    'Typed array or DataView',
    'Typed arrays other than Uint8Array and ArrayBuffer, and DataView, are not inventoried for the profile (L18).',
    'Use Uint8Array and ArrayBuffer with explicit bounds.',
  ),
  VN4015: entry(
    'Symbol.dispose protocol',
    'Symbol.dispose and Symbol.asyncDispose need resource-management lowering that is not implemented (S24).',
    'Use try/finally with an explicit cleanup call.',
  ),

  VN4016: entry(
    'Global not available in the profile',
    'The free name is not a global of the standard-library manifest for this target. Excluded (X or E) names, names restricted to other targets, and unknown names are rejected (compiler/profile/stdlib.json).',
    'Declare the value in this module, or import it from a supported module.',
  ),
  VN4017: entry(
    'Standard-library member not supported',
    'The member path is listed in the standard-library manifest and is excluded or experimental for this target (compiler/profile/stdlib.json).',
    'Use the supported alternative named in docs/compatibility/stdlib.md.',
  ),

  VN5001: entry(
    'Duplicate declaration',
    'The name is declared twice in one scope, or a lexical declaration conflicts with another declaration.',
    'Rename or remove one of the declarations.',
  ),
  VN5002: entry(
    'Assignment to const binding',
    'The binding is declared const and is assigned after initialization. The assignment throws a TypeError at run time.',
    'Declare it with let, or compute a new value instead of assigning.',
  ),
  VN5003: entry(
    'Assignment to import binding',
    'Imported bindings are read-only live views of the exporting module. Assignment throws at run time.',
    'Copy the value into a local let before changing it.',
  ),
  VN5004: entry(
    'Assignment to function expression name',
    'The name of a named function expression is immutable inside that function. Assignment throws a TypeError in strict code.',
    'Use a different local name.',
  ),

  VN7001: entry(
    'Type error',
    'The TypeScript checker reports this error. The project is checked by TypeScript 6.0.3 through the Vue language tools (vue-tsc 3.3.12), '
      + 'and the span is mapped back to the .vue or .ts file. The message names the TypeScript code (TSnnnn). Type checking is part of '
      + 'vue-native check and is skipped by --no-types (design.md sections 12.1 and 12.2).',
    'Fix the value or the declared type at the reported span.',
  ),

  VN9001: entry(
    'Internal compiler error',
    'The frontend reached an unexpected state. This is a compiler bug, not a user error.',
    'Report the input and the message to the compiler maintainers.',
  ),
});

export function isKnownCode(code) {
  return typeof code === 'string' && Object.hasOwn(CODES, code);
}

export function codeInfo(code) {
  if (!isKnownCode(code)) throw new RangeError(`unknown diagnostic code: ${String(code)}`);
  return CODES[code];
}
