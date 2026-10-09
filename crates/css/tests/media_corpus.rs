//! Media queries against Chrome's matchMedia. Each query is parsed by vn_css and evaluated in every corpus
//! environment (viewport, DPR, colour scheme, reduced motion, touch-emulated hover/pointer). Categories:
//! `clean` queries must have no diagnostic and match Chrome; `invalid` and `unknown-logic` queries carry a
//! diagnostic and still match Chrome; `unsupported` queries must carry a diagnostic (their boolean is not compared;
//! see the open questions). Corpus: tests/css/corpora/media.json (tests/css/gen/gen-media-corpus.mjs).

mod support;

use support::{Checker, Json};
use vn_css::media::{self, ColorScheme, HoverCap, MediaEnv, PointerCap};

const CORPUS: &str = include_str!("../../../tests/css/corpora/media.json");

fn env_of(e: &Json) -> MediaEnv {
    let touch = e.get("touch").bool();
    MediaEnv {
        width: e.get("width").num(),
        height: e.get("height").num(),
        color_scheme: if e.get("colorScheme").str() == "dark" { ColorScheme::Dark } else { ColorScheme::Light },
        reduced_motion: e.get("reducedMotion").bool(),
        hover: if touch { HoverCap::None } else { HoverCap::Hover },
        pointer: if touch { PointerCap::Coarse } else { PointerCap::Fine },
        dppx: e.get("dpr").num(),
    }
}

/// Compares every environment of one query. `flip` doctors the Chrome result of (query, env) when Some.
fn compare_query(c: &mut Checker, row: usize, q: &Json, envs: &[MediaEnv], flip: Option<usize>) {
    let text = q.get("q").str();
    let category = q.get("category").str();
    let (list, diags) = media::parse(text);
    if category == "clean" {
        c.check(row, &format!("{text:?} has no diagnostic"), diags.iter().map(|d| d.code).collect::<Vec<_>>(), Vec::new());
    } else {
        c.check(row, &format!("{text:?} carries a diagnostic"), !diags.is_empty(), true);
    }
    if category == "unsupported" {
        return;
    }
    for (ei, (env, chrome)) in envs.iter().zip(q.get("results").arr()).enumerate() {
        let mut want = chrome.bool();
        if flip == Some(ei) {
            want = !want;
        }
        c.check(row, &format!("{text:?} in env {ei}"), media::matches(&list, env), want);
    }
}

#[test]
fn media_queries_agree_with_chrome() {
    let corpus = Json::parse(CORPUS);
    let envs: Vec<MediaEnv> = corpus.get("envs").arr().iter().map(env_of).collect();
    assert!(envs.len() >= 4, "need several environments");
    let queries = corpus.get("queries").arr();
    assert!(queries.iter().filter(|q| q.get("category").str() == "clean").count() >= 80, "need >= 80 clean queries");
    // The environments must be what Chrome reported (viewport size and DPR were applied).
    for (env, obs) in envs.iter().zip(corpus.get("observed").arr()) {
        assert_eq!(obs.get("innerWidth").num(), env.width);
        assert_eq!(obs.get("innerHeight").num(), env.height);
        assert_eq!(obs.get("devicePixelRatio").num(), env.dppx);
    }
    let mut c = Checker::new("media");
    for (i, q) in queries.iter().enumerate() {
        compare_query(&mut c, i, q, &envs, None);
    }
    c.finish().unwrap_or_else(|e| panic!("{e}"));
}

/// Negative control: one doctored Chrome result per query is reported as a mismatch.
#[test]
fn negative_control_doctored_result_is_reported() {
    let corpus = Json::parse(CORPUS);
    let envs: Vec<MediaEnv> = corpus.get("envs").arr().iter().map(env_of).collect();
    let q = corpus.get("queries").arr().iter().find(|q| q.get("category").str() == "clean").expect("a clean query");
    let mut c = Checker::new("negative-control");
    compare_query(&mut c, 0, q, &envs, Some(0));
    assert!(c.finish().is_err(), "a flipped Chrome result must be reported");
}
