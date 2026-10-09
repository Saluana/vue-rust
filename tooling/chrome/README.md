# Headless Chrome oracle runner (WP-X1)

Zero-dependency Node 26 runner that drives the locally installed Google Chrome over the DevTools protocol (CDP
over WebSocket). It is the ground truth for DOM, CSS cascade/layout and runtime-dom behaviour: the oracle helpers
in `oracle-css.mjs` report what Chrome computes for a hand-written HTML document.

Files:
- `chrome.mjs`: `findChrome`, `launchChrome`, `CdpSession`, `openPage`, `withPage`, `Page`, `serveDirectory`.
- `oracle-css.mjs`: `measureBoxes`, `computeStyles`, `matchSelectors`.
- Tests: `tests/tooling/chrome/` (entry point `index.js`, see "Running the tests").

## Usage

```js
import { withPage, serveDirectory } from './tooling/chrome/chrome.mjs';
import { measureBoxes, computeStyles, matchSelectors } from './tooling/chrome/oracle-css.mjs';

// One browser per call, closed even when the callback throws.
await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
  await page.setContent('<!doctype html><div id="a" style="width:10px;height:5px"></div>');
  console.log(await page.evaluate(() => document.getElementById('a').getBoundingClientRect().width)); // 10
  console.log(page.consoleLog, page.exceptions);   // captured console calls and uncaught exceptions
  await page.screenshot('out/a.png');              // PNG of the viewport, absolute path returned
});

const html = '<!doctype html><style>body{margin:0}</style><div id="x" style="margin:3px;width:4px;height:5px"></div><ul><li>a</li><li>b</li></ul>';
await measureBoxes(html, '#x');
// [{ path: 'html>body>div:nth-child(1)', x: 3, y: 3, width: 4, height: 5, top: 3, right: 7, bottom: 8, left: 3 }]
await computeStyles(html, '#x', ['margin-top', 'width']);
// [{ path: 'html>body>div:nth-child(1)', styles: { 'margin-top': '3px', width: '4px' } }]
await matchSelectors(html, 'ul > li:nth-child(2)');
// ['html>body>ul:nth-child(2)>li:nth-child(2)']

// Modules and .wasm need http (file:// and about:blank pages cannot load them):
const site = await serveDirectory('build/site');  // { url, port, close() }; MIME types for html/js/mjs/css/json/wasm/png
await withPage(async (page) => { await page.goto(site.url); /* ... */ });
await site.close();
```

`openPage(options)` returns a page you close yourself with `page.close()`. Use it to share one browser across
several checks. Options for both: `headless` (default true), `userDataDir` (a caller-owned dir is never deleted),
`args` (extra Chrome flags; `--user-data-dir`, `--remote-debugging-port` and `--headless` are rejected),
`viewport` (default 800x600 at DPR 1), `timeout` (ms per operation, default 15000).

`launchChrome(options)` resolves with `{ wsUrl, port, pid, userDataDir, close() }`. It waits up to `timeout` ms for
Chrome's `DevTools listening on ws://...` line on stderr.

## Page API

- `goto(urlOrFile)`: http(s), file, about or data URLs, or an existing local path (converted to a file URL).
- `setContent(html)`: loads the string verbatim. It is not given a doctype, so include `<!doctype html>` yourself
  or the page renders in quirks mode. Its origin is about:blank (relative URLs do not resolve). Without a body margin
reset the default 8px body margin shifts every box (the example above without `body{margin:0}` measures x 11, y 8).
- `evaluate(fnOrExpression, ...args)`: a function is sent as source (`fn.toString()`), so it must be
  self-contained; an expression string runs as indirect `eval` (use an async IIFE for `await`). Promises are awaited.
- `viewport(width, height, dpr)`, `screenshot(path)` (PNG), `send(method, params)` (raw CDP on the page session).
- `consoleLog` (`{ type, text }`, CDP types such as `log`, `error`, `warning`) and `exceptions` (`{ message }`).
  Both keep entries for the page's lifetime.
- `timeout` (get/set), `pid`, `userDataDir`, `wsUrl`, `close()` (idempotent).

Page errors reject with the same `name` and `message` as in the page (TypeError, SyntaxError, and so on). A
timeout rejects with `evaluate timed out after N ms` (or the name of the operation).

## evaluate conventions

Results are JSON-encoded in the page and decoded in Node:

| In the page | Node receives |
|---|---|
| number, string, boolean, null | the same value |
| `undefined` (top level, in arrays, in objects) | `undefined` (kept) |
| `NaN`, `Infinity`, `-Infinity`, `-0` | the strings `'NaN'`, `'Infinity'`, `'-Infinity'`, `'-0'` |
| BigInt | its decimal string |
| array, plain object | array, plain object |
| function, symbol, non-plain object (Date, Map, DOM node...), cyclic value, plain key `$t` | rejects with a TypeError |

Arguments are encoded so the page receives real values: `undefined`, `NaN`, `Infinity`, `-0`, BigInt and nested
`undefined` keep their meaning.

## Chrome flags and why

`launchChrome` always passes these (WP-X1), on top of `--user-data-dir=<fresh temp dir>` (the temp dir is deleted on
close) and `--headless=new` (unless `headless: false`):

- `--headless=new`: the current headless mode, which uses the same rendering pipeline as headed Chrome.
- `--remote-debugging-port=0`: the OS picks a free port; the real port is read from stderr, so runs never collide.
- `--no-first-run`, `--no-default-browser-check`: skip first-run and default-browser UI for a fresh profile.
- `--disable-extensions`: no extension can change the pages under test.
- `--disable-gpu`: software compositing only, so no GPU process.
- `--disable-background-networking`, `--disable-sync`: no update checks, sync or other background traffic.
- `--metrics-recording-only`: no metrics upload.
- `--mute-audio`: no audio device is needed.
- `--hide-scrollbars`: scrollbars would shrink the layout viewport and change box measurements.

## Finding Chrome

`CHROME_PATH` (must be an executable file, otherwise an error is raised rather than falling back), then
`/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`, then `/usr/bin/google-chrome`,
`/usr/bin/google-chrome-stable` and `/usr/bin/chromium`. If nothing is found, the tests skip with a message on
stderr that names the reason; they never pass silently.

## Cleanup guarantees

- `close()` (also reached through `page.close()` and `withPage`, including when the callback throws) sends SIGTERM
  to the browser's process group and to every owned process, waits 3 s, then SIGKILLs the survivors, and finally
  removes the temp profile.
- Owned processes are the browser's descendants plus any process whose command line carries that launch's exact
  `--user-data-dir=<dir>` argument. Renderer, GPU and utility helpers carry it (checked on Chrome 155), so orphans
  are covered too. Other Chrome instances, including your own browser, are never signalled.
- If cleanup fails while the callback (or the launch) has already thrown, the original error is the one thrown and
  the cleanup failure is attached to it as `err.cleanupError`.
- On process exit, and on SIGINT, SIGTERM or SIGHUP, live launches are SIGKILLed and their temp profiles removed
  synchronously. This does not run after SIGKILL of the Node process itself; leftovers are then found with
  `pgrep -fl vn-chrome-`.

## Known limits

- Tested on macOS arm64 with Chrome 155 only. The Linux paths are listed but untested. Windows is not supported.
- Each oracle helper call starts its own Chrome (about 0.3 to 0.5 s). Use `openPage` for batches.
- Functions passed to `evaluate` cannot capture Node variables; their source is all the page gets.
- `file://` pages cannot load ES modules or fetch `.wasm` (Chrome origin rules): use `serveDirectory`.
- Computed values are the strings Chrome serialises (colours become `rgb(...)`); nothing is normalised.
- Geometry comes from `getBoundingClientRect()` rounded to 1/64 px.
- Console objects are described, not serialised.

## Running the tests

```sh
node --test tests/tooling/chrome/          # the acceptance command; index.js imports every suite
node --test tests/tooling/chrome/launch.test.mjs   # one file
```

Node 26 runs a directory argument as a single module, so the directory form needs `index.js`. Without a Chrome
binary, every test is reported as skipped with a stderr line naming the reason. Run time is about 13 s on the dev
machine (54 tests). After the run, `pgrep -fl "Google Chrome.*remote-debugging"` must print nothing.
