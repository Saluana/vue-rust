// The selector grammar of testHost.find (docs/dev/ui-scenario-host.md; tooling/oracle/ui/selector.mjs). Accepted and
// rejected spellings are the oracle's; each rejected one is a negative control.
use vn_uikit::selector::{parse, Cond, Selector};

#[test]
fn accepts_the_grammar_and_builds_the_parts() {
    assert_eq!(parse("div").unwrap(), Selector { tag: Some("div".into()), conds: vec![], nth_child: None });
    assert_eq!(parse("#a").unwrap().conds, vec![Cond::Id("a".into())]);
    assert_eq!(parse(".b.c").unwrap().conds, vec![Cond::Class("b".into()), Cond::Class("c".into())]);
    assert_eq!(parse("[x]").unwrap().conds, vec![Cond::Attr { name: "x".into(), value: None }]);
    assert_eq!(parse("[x=y]").unwrap().conds, vec![Cond::Attr { name: "x".into(), value: Some("y".into()) }]);
    assert_eq!(parse("[x=\"a b\"]").unwrap().conds, vec![Cond::Attr { name: "x".into(), value: Some("a b".into()) }]);
    assert_eq!(parse("[x='a]']").unwrap_err().contains("outside the oracle grammar"), true, "a quoted value cannot contain ]");
    let s = parse("li#a.b[c][d=e]:nth-child(3)").unwrap();
    assert_eq!(s.tag.as_deref(), Some("li"));
    assert_eq!(s.nth_child, Some(3));
    assert_eq!(s.conds.len(), 4);
    assert_eq!(parse("my-el").unwrap().tag.as_deref(), Some("my-el"));
}

#[test]
fn rejects_everything_outside_the_grammar_with_the_oracle_message() {
    for bad in ["", "*", "div div", "a>b", "a:hover", "[x", "[x=]", "[=x]", "#", ".", "1a", ":nth-child(2)x", "div:nth-child(n)", "A"] {
        let err = parse(bad).expect_err(bad);
        if bad.is_empty() {
            assert_eq!(err, "selector must be a non-empty string");
        } else {
            assert!(err.starts_with(&format!("selector \"{bad}\" is outside the oracle grammar")), "{bad}: {err}");
        }
    }
}
