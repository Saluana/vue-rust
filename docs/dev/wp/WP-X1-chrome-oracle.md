# WP-X1 — Headless Chrome oracle runner (CDP over WebSocket, zero npm dependencies)

Read `docs/dev/AGENT-CONTRACT.md` first. WP id `x1` (no cargo).
**Owned paths:** `tooling/chrome/**`, `tests/tooling/chrome/**`.

## Why
Browsers are the ground truth for DOM, CSS cascade/layout and runtime-dom behaviour (`getComputedStyle`,
`getBoundingClientRect`, `Element.matches`, event dispatch, `@vue/runtime-dom` in a real DOM). Later WPs use this runner to
(1) generate oracle corpora for the CSS/layout engine, (2) run the real `vue` package against scenarios for UI differential
tests, and (3) run end-to-end tests of our WASM/DOM builds. It must be dependency-free: Node 26 has global `WebSocket` and
`fetch`; Chrome is at `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome` (v155). Brave
(`/Applications/Brave Browser.app`) is also Chromium but is not required.

## Do
1. `tooling/chrome/chrome.mjs`:
   - `launchChrome({ headless: true, userDataDir?, args? })` → `{ wsUrl, port, pid, close() }`. Start Chrome with
     `--headless=new --remote-debugging-port=0 --no-first-run --no-default-browser-check --disable-extensions --disable-gpu
     --disable-background-networking --disable-sync --metrics-recording-only --mute-audio --hide-scrollbars` and a fresh
     temp `--user-data-dir` (removed on close). Parse the `DevTools listening on ws://…` line from stderr (timeout 15 s).
     Use `child_process.spawn`, never a shell string. `close()` must always kill the process tree and remove the temp dir,
     also on exceptions (`try/finally` in helpers) and on process exit signals.
   - `class CdpSession`: minimal JSON-RPC client over `WebSocket` (`send(method, params, sessionId?)`, event waiters
     `waitForEvent(name, predicate, timeout)`, console/exception capture).
   - `withPage(options, async page => …)` where `page` offers: `goto(urlOrFile)`, `setContent(html)`,
     `evaluate(fnOrExpression, ...args)` (awaits promises, returns JSON-serialisable results, throws on page exceptions with the
     original message), `evaluateHandle`-free design, `viewport(width,height,dpr)`, `screenshot(path)` (PNG), `consoleLog[]`,
     `close()`.
   - A tiny static file server helper `serveDirectory(dir)` → `{ url, close() }` (Node `http`, no dependencies, correct MIME for
     `.html .js .mjs .css .json .wasm .png`, `Cross-Origin-*` headers optional) so pages can load ES modules and `.wasm`.
2. `tooling/chrome/oracle-css.mjs`: `computeStyles(html, selectors, properties)` and `measureBoxes(html, selectors)` helpers that load
   an HTML string, run in a 800×600 viewport at DPR 1 and return, per selector match, the requested computed properties /
   `getBoundingClientRect()` numbers (rounded to 1/64 px) in document order. Plus `matchSelectors(html, selector)` →
   array of element paths (`html>body>div:nth-child(2)`) that match, using `querySelectorAll`.
3. Tests (`tests/tooling/chrome/*.test.mjs`, node:test): launch/close leaves no stray processes (check with `pgrep -f` for the
   unique temp dir) and no temp dir; `evaluate` round-trips numbers/strings/objects/arrays/undefined/NaN-as-string conventions
   (document them), a page exception is reported with its message; `setContent`+`measureBoxes` returns the exact expected
   boxes for 3 hand-computed layouts (a fixed-size flex row with gap, a padded box, an absolutely positioned child);
   `computeStyles` resolves `em/rem/%/var()/inheritance` for hand-computed expectations; `matchSelectors` for 6 selector forms;
   `serveDirectory` serves an ES module + a tiny `.wasm` (hand-assembled bytes: `00 61 73 6d 01 00 00 00`) with correct MIME and
   `WebAssembly.instantiateStreaming` succeeds in the page; timeouts: a hanging `evaluate` is rejected after the given timeout and
   the browser still closes. Skip (loudly, not silently pass) if Chrome is not found.
4. `tooling/chrome/README.md`: usage, options, flags used and why, known limits.

## Acceptance
`node --test "tests/tooling/chrome/*.test.mjs"` passes in < 60 s total; no leftover Chrome processes afterwards (`pgrep -fl "Google Chrome.*remote-debugging"` empty).
