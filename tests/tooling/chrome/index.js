// Entry point for `node --test tests/tooling/chrome/`. Node 26 runs a directory argument as a single module (it
// fails with "Cannot find module" unless the directory has an index.js), so this file imports the suite. Each test
// file also runs on its own: node --test tests/tooling/chrome/<file>.test.mjs
import './cdp.test.mjs';
import './launch.test.mjs';
import './evaluate.test.mjs';
import './oracle-css.test.mjs';
import './serve.test.mjs';
