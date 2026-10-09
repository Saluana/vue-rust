<!-- Rule (coordinator): never trace .message of an engine-generated error (TypeError/RangeError/ReferenceError/SyntaxError raised by the runtime); trace e.name / e instanceof X instead. Errors the scenario creates itself may trace name and message. -->

# Language scenario notes (WP-O1)

- Header, exactly the first two lines of every case: `// @tier N` (1 basic, 2 intermediate, 3 async/modules) then `// @covers L01,L02` (IDs from design.md section 8, L01..L21).
- Each case covers one concern. Only profile features: see the forbidden list in `tooling/oracle/lint-cases.mjs`; `node --test tests/language/oracle.test.mjs` fails on any violation.
- Golden for `cases/<rel>.js` is `golden/<rel>.js.json`, one line, exactly the bytes printed by `node tooling/oracle/run.mjs <file>`. Regenerate only with `node tooling/oracle/check-goldens.mjs --update`.
- Module-level throws and `run()` rejections use scenario-created values (`new Error('x')`, strings, objects) so goldens never contain V8 message text.
