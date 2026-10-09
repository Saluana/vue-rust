//! Media query tests with hand-written expectations: parse diagnostics by code, each supported feature against a
//! known viewport, the range forms, unit conversions (em and rem are 16 px; dpi and dpcm to dppx), and the unknown
//! feature logic (Kleene: unknown is not a match at the top level).

use vn_css::diag::code;
use vn_css::media::{matches, parse, ColorScheme, HoverCap, MediaEnv, PointerCap};

fn desktop(width: f64, height: f64) -> MediaEnv {
    MediaEnv::new(width, height)
}

fn m(q: &str, env: &MediaEnv) -> bool {
    let (list, _) = parse(q);
    matches(&list, env)
}

fn codes(q: &str) -> Vec<&'static str> {
    parse(q).1.iter().map(|d| d.code).collect()
}

#[test]
fn diagnostics_by_code() {
    assert_eq!(codes("tv"), vec![code::MEDIA_TYPE]);
    assert_eq!(codes("(color)"), vec![code::MEDIA_FEATURE]);
    assert_eq!(codes("(min-height)"), vec![code::INVALID_MEDIA_QUERY], "min- needs a value");
    assert_eq!(codes("(min-width: -1px)"), vec![code::INVALID_MEDIA_QUERY], "negative lengths are invalid (MQ3)");
    assert_eq!(codes("(width: 5)"), vec![code::INVALID_MEDIA_QUERY], "unitless non-zero lengths are invalid");
    assert_eq!(codes("(orientation: sideways)"), vec![code::INVALID_MEDIA_QUERY]);
    assert_eq!(codes("screen and"), vec![code::INVALID_MEDIA_QUERY]);
    assert_eq!(codes("not (min-width: 1px) and (width)"), vec![code::INVALID_MEDIA_QUERY]);
    assert_eq!(codes("(min-width: 1px) and not (foo)"), vec![code::INVALID_MEDIA_QUERY]);
    assert_eq!(codes("(width >= 1px) and (width >= 2px) or (width)"), vec![code::INVALID_MEDIA_QUERY], "and and or cannot mix");
    assert!(codes("screen and (min-width: 1px)").is_empty());
    assert!(codes("").is_empty(), "an empty list is valid and matches");
}

#[test]
fn spans_point_at_the_query() {
    let src = "screen, tv";
    let (_, d) = parse(src);
    assert_eq!(d.len(), 1);
    assert_eq!(&src[d[0].start..d[0].end], "tv");
}

#[test]
fn media_types() {
    let env = desktop(800.0, 600.0);
    assert!(m("all", &env));
    assert!(m("screen", &env));
    assert!(!m("print", &env), "there is no printing");
    assert!(m("not print", &env));
    assert!(m("only screen", &env));
    assert!(!m("not all", &env));
    assert!(!m("tv", &env));
    assert!(m("not tv", &env));
    assert!(m("SCREEN", &env), "media types are case-insensitive");
    assert!(!m("not screen and (min-width: 1px)", &env));
}

#[test]
fn lengths_and_range_syntax() {
    let at = |w: f64| desktop(w, 600.0);
    assert!(m("(min-width: 600px)", &at(600.0)));
    assert!(!m("(min-width: 600px)", &at(599.0)));
    assert!(m("(max-width: 600px)", &at(600.0)));
    assert!(m("(width: 600px)", &at(600.0)));
    assert!(!m("(width: 600px)", &at(601.0)));
    assert!(m("(width >= 600px)", &at(600.0)));
    assert!(!m("(width > 600px)", &at(600.0)));
    assert!(m("(600px <= width)", &at(600.0)), "value on the left is reversed");
    assert!(m("(400px < width < 800px)", &at(600.0)));
    assert!(!m("(400px < width < 800px)", &at(800.0)));
    assert!(m("(400px < width <= 800px)", &at(800.0)));
    assert!(m("(600px = width)", &at(600.0)));
    assert!(m("(width<600px) or (width>=600px)", &at(10.0)));
}

#[test]
fn em_and_rem_are_sixteen_pixels() {
    assert!(m("(min-width: 37.5em)", &desktop(600.0, 1.0)));
    assert!(!m("(min-width: 37.5em)", &desktop(599.0, 1.0)));
    assert!(m("(min-width: 50rem)", &desktop(800.0, 1.0)));
    assert!(!m("(max-width: 40em)", &desktop(800.0, 1.0)), "40em is 640px");
}

#[test]
fn orientation_is_landscape_only_when_wider_than_tall() {
    assert!(m("(orientation: landscape)", &desktop(800.0, 600.0)));
    assert!(m("(orientation: portrait)", &desktop(600.0, 800.0)));
    assert!(m("(orientation: portrait)", &desktop(600.0, 600.0)), "a square is portrait");
    assert!(!m("(orientation: landscape)", &desktop(600.0, 600.0)));
    assert!(m("(orientation: LANDSCAPE)", &desktop(800.0, 600.0)));
}

#[test]
fn colour_scheme_motion_hover_and_pointer() {
    let mut env = desktop(800.0, 600.0);
    assert!(m("(prefers-color-scheme: light)", &env));
    assert!(!m("(prefers-color-scheme: dark)", &env));
    assert!(m("(prefers-color-scheme)", &env), "any value in the boolean context");
    env.color_scheme = ColorScheme::Dark;
    assert!(m("(prefers-color-scheme: dark)", &env));
    assert!(!m("(prefers-reduced-motion: reduce)", &env));
    assert!(m("(prefers-reduced-motion: no-preference)", &env));
    assert!(!m("(prefers-reduced-motion)", &env));
    env.reduced_motion = true;
    assert!(m("(prefers-reduced-motion)", &env));
    assert!(m("(hover: hover) and (pointer: fine)", &env));
    assert!(m("(any-hover: hover)", &env));
    env.hover = HoverCap::None;
    env.pointer = PointerCap::Coarse;
    assert!(m("(hover: none) and (any-pointer: coarse)", &env));
    assert!(!m("(hover)", &env));
    assert!(!m("(pointer: none)", &env));
    assert!(m("(any-pointer)", &env));
}

#[test]
fn resolution_units_convert_to_dppx() {
    let mut env = desktop(800.0, 600.0);
    env.dppx = 1.5;
    assert!(m("(min-resolution: 1.5dppx)", &env));
    assert!(m("(resolution: 1.5x)", &env));
    assert!(m("(resolution: 144dpi)", &env), "144 dpi is 1.5 dppx");
    assert!(!m("(resolution: 96dpi)", &env));
    assert!(m("(max-resolution: 2dppx)", &env));
    assert!(!m("(min-resolution: 2dppx)", &env));
    env.dppx = 1.0;
    assert!(m("(resolution: 96dpi)", &env));
    assert!(m("(resolution)", &env));
    assert!(!m("(resolution: 0dppx)", &env));
}

#[test]
fn unknown_features_are_unknown_not_matching() {
    let env = desktop(800.0, 600.0);
    assert!(m("(min-width: 1px) or (foo: bar)", &env), "true or unknown is true");
    assert!(!m("(min-width: 1px) and (foo)", &env), "true and unknown is unknown, which does not match");
    assert!(!m("(foo: bar)", &env));
    assert!(!m("not (foo: bar)", &env), "not unknown is unknown");
    assert!(!m("(max-width: 1px) or (foo)", &env));
}

#[test]
fn comma_lists_match_when_any_query_matches() {
    let env = desktop(800.0, 600.0);
    assert!(m("print, screen and (min-width: 1px)", &env));
    assert!(m("(min-width: 9000px), (max-width: 900px)", &env));
    assert!(!m("(min-width: 9000px), print", &env));
    assert!(!m("(min-width: 5), tv", &env), "an invalid query never matches");
}

#[test]
fn media_env_defaults_are_a_mouse_desktop_at_one_dppx() {
    let env = MediaEnv::new(800.0, 600.0);
    assert_eq!(env.color_scheme, ColorScheme::Light);
    assert!(!env.reduced_motion);
    assert_eq!(env.hover, HoverCap::Hover);
    assert_eq!(env.pointer, PointerCap::Fine);
    assert_eq!(env.dppx, 1.0);
}
