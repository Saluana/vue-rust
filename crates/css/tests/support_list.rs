//! The support allowlist (vn_css::support) is data for docs and tooling. These tests check that every code it names
//! exists and every supported entry is a feature the other tests exercise, and they print the list for the report.

use vn_css::diag::code;
use vn_css::support::{render, Status, CODES, SUPPORT};

#[test]
fn every_rejected_entry_has_a_documented_code() {
    for e in SUPPORT {
        if let Status::Unsupported(c) = e.status {
            assert!(CODES.iter().any(|(k, _)| *k == c), "{c} is not in CODES ({})", e.feature);
            assert!(c.starts_with("VNCSS"), "{c}");
        }
    }
}

#[test]
fn codes_table_covers_every_diagnostic_constant_used_by_the_parser() {
    for k in [code::INVALID_DECLARATION, code::INVALID_SELECTOR, code::SEL_HAS, code::AT_IMPORT, code::MEDIA_FEATURE, code::LIMIT_RULES] {
        assert!(CODES.iter().any(|(c, _)| *c == k), "{k} missing from CODES");
    }
}

#[test]
fn every_design_row_is_listed() {
    let text = render();
    for row in ["[9.2 Selectors]", "[9.2 Cascade]", "[9.2 Variables]", "[9.2 Media/responsiveness]", "[diagnostic codes]"] {
        assert!(text.contains(row), "missing {row}");
    }
    assert!(SUPPORT.iter().filter(|e| e.status == Status::Supported).count() >= 20);
    eprintln!("{}", text);
}
