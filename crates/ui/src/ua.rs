//! The user-agent stylesheet for the supported tags, written as CSS text and parsed by vn-css once. Each rule mirrors
//! Chrome's defaults; tests/ui/style/corpora/ua.json compares them with getComputedStyle. Documented deviations:
//! `a[href]` stands for `a:-webkit-any-link` (`:any-link` is outside the subset); form controls use Arial and
//! monospace literally (Chrome's system font on the oracle machine); `img` stays `inline`, as Chrome computes it.

use std::sync::OnceLock;

use vn_css::parse::{parse_stylesheet, ParseOptions};
use vn_css::Stylesheet;

pub const UA_CSS: &str = "
html, body, div, section, main, header, footer, nav, article, aside, form, p, h1, h2, h3, h4, h5, h6, pre, ul, ol {
  display: block;
}
body { margin: 8px; }
p { margin-top: 1em; margin-bottom: 1em; }
ul, ol { margin-top: 1em; margin-bottom: 1em; padding-left: 40px; }
li { display: list-item; }
h1 { font-size: 2em; margin-top: 0.67em; margin-bottom: 0.67em; font-weight: bold; }
h2 { font-size: 1.5em; margin-top: 0.83em; margin-bottom: 0.83em; font-weight: bold; }
h3 { font-size: 1.17em; margin-top: 1em; margin-bottom: 1em; font-weight: bold; }
h4 { font-size: 1em; margin-top: 1.33em; margin-bottom: 1.33em; font-weight: bold; }
h5 { font-size: 0.83em; margin-top: 1.67em; margin-bottom: 1.67em; font-weight: bold; }
h6 { font-size: 0.67em; margin-top: 2.33em; margin-bottom: 2.33em; font-weight: bold; }
pre { display: block; font-family: monospace; font-size: 13px; white-space: pre; margin-top: 1em; margin-bottom: 1em; }
code, kbd, samp, tt { font-family: monospace; font-size: 13px; }
strong, b { font-weight: bold; }
em, i { font-style: italic; }
a[href] { color: rgb(0, 0, 238); text-decoration-line: underline; cursor: pointer; }
label { cursor: default; }
hr { display: block; margin-top: 0.5em; margin-bottom: 0.5em; border-style: inset; border-width: 1px; border-color: gray; }
button { display: inline-block; font-family: Arial; font-size: 13.3333px; padding: 1px 6px; border-style: outset; border-width: 2px; border-color: rgb(0, 0, 0); background-color: rgb(239, 239, 239); color: rgb(0, 0, 0); cursor: default; box-sizing: border-box; }
input { display: inline-block; font-family: Arial; font-size: 13.3333px; padding: 1px 2px; border-style: inset; border-width: 2px; border-color: rgb(118, 118, 118); background-color: rgb(255, 255, 255); color: rgb(0, 0, 0); cursor: text; box-sizing: content-box; }
textarea { display: inline-block; font-family: monospace; font-size: 13.3333px; padding: 2px; border-style: solid; border-width: 1px; border-color: rgb(118, 118, 118); background-color: rgb(255, 255, 255); color: rgb(0, 0, 0); cursor: text; white-space: pre-wrap; }
select { display: inline-block; font-family: Arial; font-size: 13.3333px; padding: 0px; border-style: solid; border-width: 1px; border-color: rgb(118, 118, 118); background-color: rgb(255, 255, 255); color: rgb(0, 0, 0); cursor: default; white-space: pre; box-sizing: border-box; }
[hidden] { display: none; }
";

/// The parsed user-agent sheet. The UA text is ours and is checked by tests to produce no diagnostics.
pub fn sheet() -> &'static Stylesheet {
    static SHEET: OnceLock<Stylesheet> = OnceLock::new();
    SHEET.get_or_init(|| parse_stylesheet(UA_CSS, &ParseOptions::default()).sheet)
}
