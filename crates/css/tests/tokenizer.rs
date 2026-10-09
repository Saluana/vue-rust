//! Tokenizer tests. Expected token sequences are written by hand from CSS Syntax Level 3 (section 4),
//! not produced by the code under test. Positions are checked as line and UTF-16 column.

use vn_css::token::{tokenize, Kind, Token};

fn kinds(src: &str) -> Vec<Kind> {
    tokenize(src).into_iter().map(|t| t.kind).collect()
}

fn ident(s: &str) -> Kind {
    Kind::Ident(s.to_string())
}

fn func(s: &str) -> Kind {
    Kind::Function(s.to_string())
}

fn string(s: &str) -> Kind {
    Kind::Str(s.to_string())
}

fn num(v: f64, int: bool) -> Kind {
    Kind::Number { value: v, int }
}

fn dim(v: f64, int: bool, unit: &str) -> Kind {
    Kind::Dimension { value: v, int, unit: unit.to_string() }
}

fn ws() -> Kind {
    Kind::Whitespace
}

fn check(src: &str, expected: Vec<Kind>) {
    let got = kinds(src);
    assert_eq!(got, expected, "tokens of {src:?}");
}

#[test]
fn simple_declaration_block() {
    check(
        "a{color:red}",
        vec![ident("a"), Kind::LCurly, ident("color"), Kind::Colon, ident("red"), Kind::RCurly],
    );
}

#[test]
fn strings_and_quotes() {
    check(r#""a b""#, vec![string("a b")]);
    check("'it\\'s'", vec![string("it's")]);
    check(r#""\61 ""#, vec![string("a")]);
    check("\"a\\\nb\"", vec![string("ab")]);
    check("\"a\\\r\nb\"", vec![string("ab")]);
    check("\"abc", vec![string("abc")]);
    // A newline inside a string ends it as a bad-string; the newline is left for the next token.
    check("\"ab\ncd\"", vec![Kind::BadStr, ws(), ident("cd"), string("")]);
}

#[test]
fn hashes() {
    check("#fff", vec![Kind::Hash { value: "fff".into(), id: true }]);
    check("#1ab", vec![Kind::Hash { value: "1ab".into(), id: false }]);
    check("#\\31 x", vec![Kind::Hash { value: "1x".into(), id: true }]);
    check("# a", vec![Kind::Delim('#'), ws(), ident("a")]);
}

#[test]
fn numbers_percentages_dimensions() {
    check("10px", vec![dim(10.0, true, "px")]);
    check("1.5e3%", vec![Kind::Percentage(1500.0)]);
    check("50%", vec![Kind::Percentage(50.0)]);
    check("-50%", vec![Kind::Percentage(-50.0)]);
    check("+.5", vec![num(0.5, false)]);
    check("-.5e-1", vec![num(-0.05, false)]);
    check("1e3", vec![num(1000.0, false)]);
    check("1E-2px", vec![dim(0.01, false, "px")]);
    check("-1px", vec![dim(-1.0, true, "px")]);
    check("1e", vec![dim(1.0, true, "e")]);
    check("1e+", vec![dim(1.0, true, "e"), Kind::Delim('+')]);
    check("1-2", vec![num(1.0, true), num(-2.0, true)]);
    check("1.", vec![num(1.0, true), Kind::Delim('.')]);
    check(".5.5", vec![num(0.5, false), num(0.5, false)]);
    check("1.5", vec![num(1.5, false)]);
}

#[test]
fn identifiers_and_dashes() {
    check("a-1", vec![ident("a-1")]);
    check("--x", vec![ident("--x")]);
    check("-x", vec![ident("-x")]);
    check("-", vec![Kind::Delim('-')]);
    check("é", vec![ident("é")]);
    check("€1", vec![ident("€1")]);
    check("a\0b", vec![ident("a\u{FFFD}b")]);
}

#[test]
fn at_keywords_and_delimiters() {
    check("@media", vec![Kind::AtKeyword("media".into())]);
    check("@", vec![Kind::Delim('@')]);
    check("@1", vec![Kind::Delim('@'), num(1.0, true)]);
    check("<", vec![Kind::Delim('<')]);
    check("<!-", vec![Kind::Delim('<'), Kind::Delim('!'), Kind::Delim('-')]);
    check("*|a", vec![Kind::Delim('*'), Kind::Delim('|'), ident("a")]);
    check(":;,", vec![Kind::Colon, Kind::Semicolon, Kind::Comma]);
    check("[(){}]", vec![Kind::LSquare, Kind::LParen, Kind::RParen, Kind::LCurly, Kind::RCurly, Kind::RSquare]);
}

#[test]
fn cdo_cdc() {
    check("-->", vec![Kind::Cdc]);
    check("<!--", vec![Kind::Cdo]);
    check("<!-- a -->", vec![Kind::Cdo, ws(), ident("a"), ws(), Kind::Cdc]);
}

#[test]
fn urls() {
    check("url(a.png)", vec![Kind::Url("a.png".into())]);
    check("url( \"a.png\" )", vec![func("url"), ws(), string("a.png"), ws(), Kind::RParen]);
    check("URL('a')", vec![func("URL"), string("a"), Kind::RParen]);
    check("url(a b)", vec![Kind::BadUrl]);
    check("url(a\"b)", vec![Kind::BadUrl]);
    check("url(a\nb)", vec![Kind::BadUrl]);
    check("url(a\\29 b)", vec![Kind::Url("a)b".into())]);
    check("url(", vec![Kind::Url(String::new())]);
    check("url(a(b)", vec![Kind::BadUrl]);
    check("url(a\u{7}b)", vec![Kind::BadUrl]);
}

#[test]
fn functions_and_whitespace() {
    check("rgb(1,2)", vec![func("rgb"), num(1.0, true), Kind::Comma, num(2.0, true), Kind::RParen]);
    check("  \t\n", vec![ws()]);
    check("a \r\n b", vec![ident("a"), ws(), ident("b")]);
}

#[test]
fn escapes_in_identifiers() {
    check("\\31 23", vec![ident("123")]);
    check("\\0", vec![ident("\u{FFFD}")]);
    check("\\D800", vec![ident("\u{FFFD}")]);
    check("\\110000", vec![ident("\u{FFFD}")]);
    check("\\\n", vec![Kind::Delim('\\'), ws()]);
    check("a\\", vec![ident("a\u{FFFD}")]);
    check("\\61 b", vec![ident("ab")]);
    check("\\72gb(", vec![func("rgb")]);
}

#[test]
fn comments_are_not_tokens() {
    check("/* c */a", vec![ident("a")]);
    check("a/**/b", vec![ident("a"), ident("b")]);
    check("a/*", vec![ident("a")]);
    check("/*/a*/b", vec![ident("b")]);
    check("a/**/ /**/b", vec![ident("a"), ws(), ident("b")]);
}

#[test]
fn positions_are_line_and_utf16_column() {
    let toks: Vec<Token> = tokenize("a\r\nb");
    let pos: Vec<(u32, u32)> = toks.iter().map(|t| (t.line, t.col)).collect();
    assert_eq!(pos, vec![(1, 0), (1, 1), (2, 0)]);

    let toks = tokenize("é b");
    let pos: Vec<(u32, u32)> = toks.iter().map(|t| (t.line, t.col)).collect();
    assert_eq!(pos, vec![(1, 0), (1, 1), (1, 2)]);

    // U+1F600 is a name code point (non-ASCII), so the emoji and the space are separate tokens.
    let toks = tokenize("😀 b");
    assert_eq!((toks[0].line, toks[0].col), (1, 0));
    assert_eq!((toks[1].line, toks[1].col), (1, 2));
    assert_eq!((toks[2].line, toks[2].col), (1, 3));
    assert_eq!((toks[2].start, toks[2].end), (5, 6));

    let toks = tokenize("a\rb");
    assert_eq!((toks[2].line, toks[2].col), (2, 0));

    let toks = tokenize("/*\n*/a");
    assert_eq!((toks[0].line, toks[0].col), (2, 2));
}

#[test]
fn byte_spans_cover_the_source_text() {
    let src = "p{ color : #fff }";
    let toks = tokenize(src);
    for t in &toks {
        assert!(t.start < t.end && t.end <= src.len());
    }
    assert_eq!(toks[0].text(src), "p");
    assert_eq!(toks[2].text(src), " ");
    assert_eq!(toks.last().expect("tokens").text(src), "}");
}

#[test]
fn deep_nesting_and_long_input_do_not_recurse() {
    let deep = "(".repeat(200_000) + &")".repeat(200_000);
    let toks = tokenize(&deep);
    assert_eq!(toks.len(), 400_000);
    let long = "a ".repeat(500_000);
    assert_eq!(tokenize(&long).len(), 1_000_000);
}
