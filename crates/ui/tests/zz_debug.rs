mod support;
use support::{append, element, no_listeners};
use vn_css::media::MediaEnv;
use vn_dom::Document;
use vn_ui::{compute_styles, NoStates, Sheet};
#[test]
fn debug_nth() {
    let mut doc = Document::new();
    let html = element(&mut doc, "html", "");
    let body = element(&mut doc, "body", "");
    let ul = element(&mut doc, "ul", "");
    let div = element(&mut doc, "div", "");
    let input = element(&mut doc, "input", "");
    doc.set_attribute(input, "type", "checkbox").unwrap();
    let ul2 = element(&mut doc, "ul", "");
    append(&mut doc, ul, div);
    append(&mut doc, ul, input);
    append(&mut doc, ul, ul2);
    append(&mut doc, body, ul);
    append(&mut doc, html, body);
    let top = doc.document();
    append(&mut doc, top, html);
    let sheet = Sheet::parse(":nth-child(2n+1) { text-align: right !important }");
    let tree = compute_styles(&mut doc, std::slice::from_ref(&sheet), &MediaEnv::new(800.0, 600.0), &NoStates);
    for e in &tree.elements {
        eprintln!("node {:?} text-align {:?} parent {:?}", e.node, e.style.get("text-align"), e.parent);
    }
}

#[test]
fn debug_parse() {
    for sel in [":nth-child(2n+1)", "div:nth-child(2n+1)", ":nth-child(odd)", "*:nth-child(2n + 1)"] {
        let list = vn_css::selector::parse(sel);
        eprintln!("SEL {sel} => {:?}", list.map(|l| l.selectors.iter().map(|s| format!("{:?}", s.parts)).collect::<Vec<_>>()));
    }
}
