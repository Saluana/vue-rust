//! Cascade and custom properties against Chrome (tests/css/corpora/cascade.json, 90 fixtures), and var() serialisation
//! against Chrome (tests/css/corpora/serialize.json: a 2592-row boundary grid and a directed table).
//! Per element: custom properties --a..--e, --dsp, --vis, --pos are compared with getPropertyValue ('' when Chrome has
//! no value). display, position and visibility are compared when the cascade has a winner: the winner's value after
//! var() substitution must equal the computed value. An invalid value (empty, or invalid at computed-value time)
//! means the property's initial value: inline for display, static for position, and no check for visibility
//! (inherited). For display, a block box is skipped when position is absolute or fixed (blockified in Chrome).

mod support;

use support::{fake, Checker, Doc, Json};
use vn_css::cascade::{cascade, CascadedDeclarations, Origin};
use vn_css::custom::{computed_custom_properties, CustomProperties};
use vn_css::media::{ColorScheme, HoverCap, MediaEnv, PointerCap};
use vn_css::matching::ElementState;
use vn_css::parse::{parse_declarations, parse_stylesheet, ParseOptions};
use vn_css::diag::code;
use vn_css::{Declaration, Stylesheet};

const CASCADE: &str = include_str!("../../../tests/css/corpora/cascade.json");
const SERIALIZE: &str = include_str!("../../../tests/css/corpora/serialize.json");

const CUSTOM: [&str; 8] = ["--a", "--b", "--c", "--d", "--e", "--dsp", "--vis", "--pos"];

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

/// The outcome of a keyword property for one element.
#[derive(Debug, PartialEq)]
enum Keyword {
    /// No author declaration wins.
    Absent,
    /// The winner is invalid at computed-value time, or its value is empty.
    Invalid,
    Value(String),
}

fn keyword(cas: &CascadedDeclarations, name: &str, custom: &CustomProperties) -> Keyword {
    match cas.winner(name) {
        None => Keyword::Absent,
        Some(c) => match custom.substitute(&c.decl.value) {
            Ok(t) if t.text().is_empty() => Keyword::Invalid,
            Ok(t) => Keyword::Value(t.text().to_string()),
            Err(_) => Keyword::Invalid,
        },
    }
}

fn expected_keyword(k: &Keyword, initial: &str) -> Option<String> {
    match k {
        Keyword::Absent => None,
        Keyword::Invalid => Some(initial.to_string()),
        Keyword::Value(v) => Some(v.clone()),
    }
}

/// Checks one fixture. `doctor` replaces the Chrome value of (element, custom name) when Some.
fn check_fixture(c: &mut Checker, fx: &Json, envs: &[Json], doctor: Option<(usize, &str, &str)>) -> usize {
    let id = fx.get("id").int() as usize;
    let css = fx.get("css").str();
    let env_name = fx.get("env").str();
    let env = env_of(envs.iter().find(|e| e.get("name").str() == env_name).expect("known env"));
    let parsed = parse_stylesheet(css, &ParseOptions::default());
    assert!(parsed.diagnostics.is_empty(), "fixture {id} has diagnostics: {:?}", parsed.diagnostics);
    let doc = Doc::from_json(fx.get("root"));
    let n = doc.len();
    let inlines: Vec<Vec<Declaration>> = (0..n)
        .map(|i| {
            let style = doc.nodes[i].attrs.iter().find(|(k, _)| k == "style").map(|(_, v)| v.clone());
            match style {
                Some(s) => {
                    let (decls, diags) = parse_declarations(&s);
                    assert!(diags.is_empty(), "fixture {id} element {i} style attribute: {diags:?}");
                    decls
                }
                None => Vec::new(),
            }
        })
        .collect();
    let sheets: [(&Stylesheet, Origin); 1] = [(&parsed.sheet, Origin::Author)];
    let cascades: Vec<CascadedDeclarations> = (0..n).map(|i| cascade(&doc.el(i), &sheets, &env, &inlines[i])).collect();
    let computed = fx.get("computed").arr();
    assert_eq!(computed.len(), n, "fixture {id}: element count");
    let mut checked = 0;
    for i in 0..n {
        let mut chain_idx = Vec::new();
        let mut cur = Some(i);
        while let Some(j) = cur {
            chain_idx.push(j);
            cur = doc.nodes[j].parent;
        }
        chain_idx.reverse();
        let chain: Vec<&CascadedDeclarations> = chain_idx.iter().map(|&j| &cascades[j]).collect();
        let custom = computed_custom_properties(&chain);
        let want = &computed[i];
        for name in CUSTOM {
            let mut expect = want.get("custom").get(name).str().to_string();
            if let Some((de, dn, dv)) = doctor {
                if de == i && dn == name {
                    expect = dv.to_string();
                }
            }
            let got = custom.get(name).map(|t| t.text().to_string()).unwrap_or_default();
            c.check(id, &format!("fixture {id} element {i} {name}"), got, expect);
            checked += 1;
        }
        let position = keyword(&cascades[i], "position", &custom);
        let position_value = match &position {
            Keyword::Value(v) => v.clone(),
            _ => String::new(),
        };
        let blockified = matches!(position_value.as_str(), "absolute" | "fixed");
        if let Some(want_pos) = expected_keyword(&position, "static") {
            c.check(id, &format!("fixture {id} element {i} position"), want.get("position").str().to_string(), want_pos);
            checked += 1;
        }
        // The root element is blockified by Chrome (display inline-block becomes block): not modelled here.
        let root = doc.nodes[i].parent.is_none();
        if !blockified && !root {
            if let Some(want_display) = expected_keyword(&keyword(&cascades[i], "display", &custom), "inline") {
                c.check(id, &format!("fixture {id} element {i} display"), want.get("display").str().to_string(), want_display);
                checked += 1;
            }
        }
        if let Keyword::Value(v) = keyword(&cascades[i], "visibility", &custom) {
            c.check(id, &format!("fixture {id} element {i} visibility"), want.get("visibility").str().to_string(), v);
            checked += 1;
        }
    }
    checked
}

#[test]
fn cascade_agrees_with_chrome() {
    let corpus = Json::parse(CASCADE);
    let envs = corpus.get("envs").arr();
    let fixtures = corpus.get("fixtures").arr();
    assert!(fixtures.len() >= 80, "corpus has {} fixtures, need >= 80", fixtures.len());
    let mut c = Checker::new("cascade");
    let mut checked = 0;
    for fx in fixtures {
        checked += check_fixture(&mut c, fx, envs, None);
    }
    assert!(checked > 5000, "only {checked} comparisons");
    c.finish().unwrap_or_else(|e| panic!("{e}"));
}

/// Negative control: a doctored Chrome custom property value for one element must be reported.
#[test]
fn negative_control_doctored_custom_value_is_reported() {
    let corpus = Json::parse(CASCADE);
    let envs = corpus.get("envs").arr();
    let fx = &corpus.get("fixtures").arr()[0];
    let mut c = Checker::new("negative-control");
    check_fixture(&mut c, fx, envs, Some((0, "--a", "DOCTORED")));
    assert!(c.finish().is_err(), "a doctored Chrome value must be reported");
}

/// Serialisation: each boundary row and each directed case is one rule on one element.
fn serialize_check(c: &mut Checker, row: usize, css: &str, id: &str, want_x: &str, want_p: Option<&str>) {
    let parsed = parse_stylesheet(css, &ParseOptions::default());
    // env() is unsupported: the declaration is dropped with VNCSS2011 and Chrome's evaluated value is not compared.
    if css.contains("env(") {
        let codes: Vec<&str> = parsed.diagnostics.iter().map(|d| d.code).collect();
        c.check(row, &format!("{css:?} env() diagnostic"), codes, vec![code::UNSUPPORTED_FUNCTION]);
        return;
    }
    let doc = fake("div", &[("id", id)], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let sheets: [(&Stylesheet, Origin); 1] = [(&parsed.sheet, Origin::Author)];
    let cas = cascade(&doc.el(0), &sheets, &env, &[]);
    let custom = computed_custom_properties(&[&cas]);
    let got_x = custom.get("--x").map(|t| t.text().to_string()).unwrap_or_default();
    c.check(row, &format!("{css:?} --x"), got_x, want_x.to_string());
    if let Some(p) = want_p {
        let got_p = custom.get("--p").map(|t| t.text().to_string()).unwrap_or_default();
        c.check(row, &format!("{css:?} --p"), got_p, p.to_string());
    }
}

#[test]
fn var_serialisation_agrees_with_chrome() {
    let corpus = Json::parse(SERIALIZE);
    let grid = corpus.get("grid").arr();
    let directed = corpus.get("directed").arr();
    assert!(grid.len() >= 2000, "grid has {} rows", grid.len());
    let mut c = Checker::new("serialize");
    for (i, r) in grid.iter().enumerate() {
        serialize_check(&mut c, i, r.get("css").str(), r.get("id").str(), r.get("out").str(), Some(r.get("pv").str()));
    }
    for (i, r) in directed.iter().enumerate() {
        serialize_check(&mut c, 100_000 + i, r.get("css").str(), r.get("id").str(), r.get("out").str(), None);
    }
    c.finish().unwrap_or_else(|e| panic!("{e}"));
}

/// Negative control: a doctored grid row (an extra space) must be reported.
#[test]
fn negative_control_doctored_serialisation_is_reported() {
    let corpus = Json::parse(SERIALIZE);
    let row = &corpus.get("grid").arr()[0];
    let mut c = Checker::new("negative-control");
    let doctored = format!("{} ", row.get("out").str());
    serialize_check(&mut c, 0, row.get("css").str(), row.get("id").str(), &doctored, None);
    assert!(c.finish().is_err(), "a doctored serialisation must be reported");
}
