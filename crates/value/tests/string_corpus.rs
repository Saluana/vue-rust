//! Compares vn-value's `JsStr` methods with V8 (String.prototype.* on Node 26) using
//! tests/language/corpora/string.json. Strings are arrays of UTF-16 code units. The "strings" table
//! holds every string argument; rows refer to it by index. Positions and counts are the already-clamped
//! usize values the API takes, and a thrown RangeError is {"throw":"RangeError"}.

mod support;

use std::cmp::Ordering;

use support::{rows, Checker, Json};
use vn_value::JsStr;

const CORPUS: &str = include_str!("../../../tests/language/corpora/string.json");

fn doc() -> Json {
    Json::parse(CORPUS)
}

fn strings(doc: &Json) -> Vec<JsStr> {
    doc.get("strings").arr().iter().map(|u| JsStr::from_vec(u.units())).collect()
}

fn js(v: &Json) -> JsStr {
    JsStr::from_vec(v.units())
}

fn sid(tab: &[JsStr], v: &Json) -> JsStr {
    tab[v.int() as usize].clone()
}

fn opt_usize(v: &Json) -> Option<usize> {
    if v.is_null() { None } else { Some(v.int() as usize) }
}

fn expected_ok_or_range(v: &Json) -> Result<JsStr, String> {
    match v.thrown() {
        Some(name) => Err(name.to_string()),
        None => Ok(js(v)),
    }
}

fn finish(c: Checker) {
    let k = c.finish().unwrap_or_else(|e| panic!("{e}"));
    assert!(k > 0, "corpus table is empty");
}

#[test]
fn to_lower_and_upper_case_match_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut lower = Checker::new("toLowerCase");
    let mut upper = Checker::new("toUpperCase");
    for (i, row) in rows(&doc, "toLowerCase").iter().enumerate() {
        let r = row.arr();
        let s = sid(&tab, &r[0]);
        lower.check(i, &format!("toLowerCase({s:?})"), s.to_lower_case(), js(&r[1]));
    }
    for (i, row) in rows(&doc, "toUpperCase").iter().enumerate() {
        let r = row.arr();
        let s = sid(&tab, &r[0]);
        upper.check(i, &format!("toUpperCase({s:?})"), s.to_upper_case(), js(&r[1]));
    }
    finish(lower);
    finish(upper);
}

#[test]
fn trim_variants_match_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("trim");
    for (table, f) in [
        ("trim", JsStr::trim as fn(&JsStr) -> JsStr),
        ("trimStart", JsStr::trim_start),
        ("trimEnd", JsStr::trim_end),
    ] {
        for (i, row) in rows(&doc, table).iter().enumerate() {
            let r = row.arr();
            let s = sid(&tab, &r[0]);
            c.check(i, &format!("{table}({s:?})"), f(&s), js(&r[1]));
        }
    }
    finish(c);
}

#[test]
fn well_formedness_matches_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("isWellFormed");
    for (i, row) in rows(&doc, "isWellFormed").iter().enumerate() {
        let r = row.arr();
        let s = sid(&tab, &r[0]);
        c.check(i, &format!("isWellFormed({s:?})"), s.is_well_formed(), r[1].bool());
    }
    finish(c);
    let mut c = Checker::new("toWellFormed");
    for (i, row) in rows(&doc, "toWellFormed").iter().enumerate() {
        let r = row.arr();
        let s = sid(&tab, &r[0]);
        c.check(i, &format!("toWellFormed({s:?})"), s.to_well_formed(), js(&r[1]));
    }
    finish(c);
}

#[test]
fn array_index_matches_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("arrayIndex");
    for (i, row) in rows(&doc, "arrayIndex").iter().enumerate() {
        let r = row.arr();
        let s = sid(&tab, &r[0]);
        let want = opt_usize(&r[1]).map(|v| v as u32);
        c.check(i, &format!("arrayIndex({s:?})"), s.array_index(), want);
    }
    finish(c);
}

#[test]
fn code_points_iterator_matches_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("codePoints");
    for (i, row) in rows(&doc, "codePoints").iter().enumerate() {
        let r = row.arr();
        let s = sid(&tab, &r[0]);
        let got: Vec<JsStr> = s.code_points().collect();
        let want: Vec<JsStr> = r[1].arr().iter().map(js).collect();
        c.check(i, &format!("[...{s:?}]"), got, want);
    }
    finish(c);
}

#[test]
fn code_unit_and_code_point_at_match_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("codeUnitAt");
    for (i, row) in rows(&doc, "codeUnitAt").iter().enumerate() {
        let r = row.arr();
        let s = sid(&tab, &r[0]);
        let at = r[1].int() as usize;
        let want = if r[2].is_null() { None } else { Some(r[2].int() as u16) };
        c.check(i, &format!("{s:?}.charCodeAt({at})"), s.code_unit_at(at), want);
    }
    finish(c);
    let mut c = Checker::new("codePointAt");
    for (i, row) in rows(&doc, "codePointAt").iter().enumerate() {
        let r = row.arr();
        let s = sid(&tab, &r[0]);
        let at = r[1].int() as usize;
        let want = if r[2].is_null() {
            None
        } else {
            Some((r[2].arr()[0].int() as u32, r[2].arr()[1].int() as usize))
        };
        c.check(i, &format!("{s:?}.codePointAt({at})"), s.code_point_at(at), want);
    }
    finish(c);
}

#[test]
fn substring_matches_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("substring");
    for (i, row) in rows(&doc, "substring").iter().enumerate() {
        let r = row.arr();
        let s = sid(&tab, &r[0]);
        let (a, b) = (r[1].int() as usize, r[2].int() as usize);
        c.check(i, &format!("{s:?}.substring({a}, {b})"), s.substring(a, b), js(&r[3]));
    }
    finish(c);
}

#[test]
fn search_methods_match_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("indexOf");
    for (i, row) in rows(&doc, "indexOf").iter().enumerate() {
        let r = row.arr();
        let (s, n, f) = (sid(&tab, &r[0]), sid(&tab, &r[1]), r[2].int() as usize);
        c.check(i, &format!("{s:?}.indexOf({n:?}, {f})"), s.index_of(&n, f), opt_usize(&r[3]));
    }
    finish(c);
    let mut c = Checker::new("lastIndexOf");
    for (i, row) in rows(&doc, "lastIndexOf").iter().enumerate() {
        let r = row.arr();
        let (s, n, f) = (sid(&tab, &r[0]), sid(&tab, &r[1]), r[2].int() as usize);
        c.check(i, &format!("{s:?}.lastIndexOf({n:?}, {f})"), s.last_index_of(&n, f), opt_usize(&r[3]));
    }
    finish(c);
    let mut c = Checker::new("startsWith");
    for (i, row) in rows(&doc, "startsWith").iter().enumerate() {
        let r = row.arr();
        let (s, n, p) = (sid(&tab, &r[0]), sid(&tab, &r[1]), r[2].int() as usize);
        c.check(i, &format!("{s:?}.startsWith({n:?}, {p})"), s.starts_with(&n, p), r[3].bool());
    }
    finish(c);
    let mut c = Checker::new("endsWith");
    for (i, row) in rows(&doc, "endsWith").iter().enumerate() {
        let r = row.arr();
        let (s, n, p) = (sid(&tab, &r[0]), sid(&tab, &r[1]), r[2].int() as usize);
        c.check(i, &format!("{s:?}.endsWith({n:?}, {p})"), s.ends_with(&n, p), r[3].bool());
    }
    finish(c);
    let mut c = Checker::new("includes");
    for (i, row) in rows(&doc, "includes").iter().enumerate() {
        let r = row.arr();
        let (s, n, p) = (sid(&tab, &r[0]), sid(&tab, &r[1]), r[2].int() as usize);
        c.check(i, &format!("{s:?}.includes({n:?}, {p})"), s.includes(&n, p), r[3].bool());
    }
    finish(c);
}

#[test]
fn padding_and_repeat_match_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("padStart");
    for (i, row) in rows(&doc, "padStart").iter().enumerate() {
        let r = row.arr();
        let (s, t, fill) = (sid(&tab, &r[0]), r[1].int() as usize, sid(&tab, &r[2]));
        let actual = s.pad_start(t, &fill).map_err(|_| "RangeError".to_string());
        c.check(i, &format!("{s:?}.padStart({t}, {fill:?})"), actual, expected_ok_or_range(&r[3]));
    }
    finish(c);
    let mut c = Checker::new("padEnd");
    for (i, row) in rows(&doc, "padEnd").iter().enumerate() {
        let r = row.arr();
        let (s, t, fill) = (sid(&tab, &r[0]), r[1].int() as usize, sid(&tab, &r[2]));
        let actual = s.pad_end(t, &fill).map_err(|_| "RangeError".to_string());
        c.check(i, &format!("{s:?}.padEnd({t}, {fill:?})"), actual, expected_ok_or_range(&r[3]));
    }
    finish(c);
    let mut c = Checker::new("repeat");
    for (i, row) in rows(&doc, "repeat").iter().enumerate() {
        let r = row.arr();
        let (s, n) = (sid(&tab, &r[0]), r[1].int() as usize);
        let actual = s.repeat(n).map_err(|_| "RangeError".to_string());
        c.check(i, &format!("{s:?}.repeat({n})"), actual, expected_ok_or_range(&r[2]));
    }
    finish(c);
}

#[test]
fn split_matches_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("split");
    for (i, row) in rows(&doc, "split").iter().enumerate() {
        let r = row.arr();
        let (s, sep) = (sid(&tab, &r[0]), sid(&tab, &r[1]));
        // A null limit is undefined in JavaScript, which means no limit (2^32 - 1).
        let limit = if r[2].is_null() { u32::MAX } else { r[2].int() as u32 };
        let want: Vec<JsStr> = r[3].arr().iter().map(js).collect();
        c.check(i, &format!("{s:?}.split({sep:?}, {limit})"), s.split_by_string(&sep, limit), want);
    }
    finish(c);
}

#[test]
fn replace_and_replace_all_match_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("replace");
    for (i, row) in rows(&doc, "replace").iter().enumerate() {
        let r = row.arr();
        let (s, p, rep) = (sid(&tab, &r[0]), sid(&tab, &r[1]), sid(&tab, &r[2]));
        c.check(i, &format!("{s:?}.replace({p:?}, {rep:?})"), s.replace_first(&p, &rep), js(&r[3]));
    }
    finish(c);
    let mut c = Checker::new("replaceAll");
    for (i, row) in rows(&doc, "replaceAll").iter().enumerate() {
        let r = row.arr();
        let (s, p, rep) = (sid(&tab, &r[0]), sid(&tab, &r[1]), sid(&tab, &r[2]));
        c.check(i, &format!("{s:?}.replaceAll({p:?}, {rep:?})"), s.replace_all(&p, &rep), js(&r[3]));
    }
    finish(c);
}

#[test]
fn concat_and_compare_match_v8() {
    let doc = doc();
    let tab = strings(&doc);
    let mut c = Checker::new("concat");
    for (i, row) in rows(&doc, "concat").iter().enumerate() {
        let r = row.arr();
        let (a, b) = (sid(&tab, &r[0]), sid(&tab, &r[1]));
        c.check(i, &format!("{a:?}.concat({b:?})"), a.concat(&b), js(&r[2]));
    }
    finish(c);
    let mut c = Checker::new("compare");
    for (i, row) in rows(&doc, "compare").iter().enumerate() {
        let r = row.arr();
        let (a, b) = (sid(&tab, &r[0]), sid(&tab, &r[1]));
        let want = match r[2].int() {
            -1 => Ordering::Less,
            0 => Ordering::Equal,
            _ => Ordering::Greater,
        };
        c.check(i, &format!("{a:?} < {b:?}"), a.compare(&b), want);
    }
    finish(c);
}

#[test]
fn from_char_code_and_from_code_point_match_v8() {
    let doc = doc();
    let mut c = Checker::new("fromCharCode");
    for (i, row) in rows(&doc, "fromCharCode").iter().enumerate() {
        let r = row.arr();
        let units = r[0].units();
        c.check(i, &format!("String.fromCharCode{units:?}"), JsStr::from_char_code(&units), js(&r[1]));
    }
    finish(c);
    let mut c = Checker::new("fromCodePoint");
    for (i, row) in rows(&doc, "fromCodePoint").iter().enumerate() {
        let r = row.arr();
        let cp = r[0].int() as u32;
        let want = if r[1].is_null() { None } else { Some(js(&r[1])) };
        c.check(i, &format!("String.fromCodePoint({cp})"), JsStr::from_code_point(cp), want);
    }
    finish(c);
}

/// Negative control: mutated corpus rows must be reported, so the comparison cannot pass vacuously.
#[test]
fn negative_control_mutated_corpus_rows_are_reported() {
    let doc = doc();
    let tab = strings(&doc);
    let row = rows(&doc, "toUpperCase")[1].arr().to_vec();
    let subject = sid(&tab, &row[0]);
    let want = js(&row[1]);
    let mut wrong_units = want.units().to_vec();
    wrong_units.push(0x41);
    let mut c = Checker::new("negative control");
    c.check(0, "toUpperCase, correct expectation", subject.to_upper_case(), want);
    c.check(1, "toUpperCase, extra unit", subject.to_upper_case(), JsStr::from_vec(wrong_units));
    c.check(2, "trim keeps U+180E", JsStr::from_str("\u{180e}a").trim(), JsStr::from_str("a"));
    let err = c.finish().expect_err("mutated expectations must be reported");
    assert!(err.contains("2 of 3 cases mismatch"), "{err}");
    assert!(err.contains("toUpperCase, extra unit"), "{err}");
    assert!(err.contains("trim keeps U+180E"), "{err}");
}

/// Positive control: a correct expectation passes, and lone surrogates compare by code unit.
#[test]
fn positive_control_lone_surrogates_round_trip() {
    let mut c = Checker::new("positive control");
    c.check(0, "lone high surrogate", JsStr::from_units(&[0xD800]).len(), 1);
    c.check(1, "pair is one code point", JsStr::from_str("\u{1F600}").code_point_at(0), Some((0x1F600, 2)));
    assert_eq!(c.finish(), Ok(2));
}
