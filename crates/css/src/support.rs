//! The CSS subset as data: every selector form, at-rule, media feature and cascade rule, supported or
//! rejected, with the design row it belongs to (planning/vue-native-framework/design.md section 9.2). Docs and
//! tooling print this list (`render`); the compiler validates `<style>` blocks against the same decisions.

use crate::diag::code;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Supported,
    /// Rejected with this diagnostic code.
    Unsupported(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// design.md section 9.2 row this entry belongs to.
    pub design: &'static str,
    pub feature: &'static str,
    pub status: Status,
}

const SEL: &str = "9.2 Selectors";
const CAS: &str = "9.2 Cascade";
const VAR: &str = "9.2 Variables";
const MQ: &str = "9.2 Media/responsiveness";

const fn ok(design: &'static str, feature: &'static str) -> Entry {
    Entry { design, feature, status: Status::Supported }
}

const fn no(design: &'static str, feature: &'static str, c: &'static str) -> Entry {
    Entry { design, feature, status: Status::Unsupported(c) }
}

/// The allowlist. Supported entries are exactly what the parser, matcher, media and cascade accept.
pub const SUPPORT: &[Entry] = &[
    ok(SEL, "type selectors (ASCII case-insensitive for HTML)"),
    ok(SEL, "universal selector *"),
    ok(SEL, "class .name and id #name"),
    ok(SEL, "attribute [a], [a=v], [a~=v], [a|=v], [a^=v], [a$=v], [a*=v] with the i flag"),
    ok(SEL, "combinators: descendant, >, +, ~"),
    ok(SEL, ":hover :active :focus :focus-visible :focus-within :disabled :enabled :checked (state from the element)"),
    ok(SEL, ":first-child :last-child :only-child :empty :root"),
    ok(SEL, ":nth-child(An+B) :nth-last-child(An+B) with odd and even"),
    ok(SEL, ":not(compound selector list)"),
    no(SEL, ":has()", code::SEL_HAS),
    no(SEL, ":is() :where() :matches()", code::SEL_IS_WHERE),
    no(SEL, ":visited :link :any-link :target :target-within", code::SEL_LINK_HISTORY),
    no(SEL, "pseudo-elements ::before ::after ::first-line ::first-letter and legacy :before :after", code::SEL_PSEUDO_ELEMENT),
    no(SEL, ":lang() :dir()", code::SEL_LANG_DIR),
    no(SEL, ":nth-of-type() :nth-last-of-type() :first-of-type :last-of-type :only-of-type", code::SEL_OF_TYPE),
    no(SEL, "namespace prefixes ns|a *|a |a", code::SEL_NAMESPACE),
    no(SEL, "complex selectors (combinators) inside :not()", code::SEL_COMPLEX_NOT),
    no(SEL, "column combinator ||", code::SEL_COLUMN),
    no(SEL, ":nth-child(An+B of S)", code::SEL_NTH_OF),
    no(SEL, "every other pseudo-class (for example :scope, :defined, :placeholder-shown)", code::SEL_PSEUDO_CLASS),
    no(SEL, "nesting selector & and nested style rules", code::NESTING_SELECTOR),
    no(SEL, "attribute selector flags other than i", code::INVALID_ATTRIBUTE_FLAG),
    ok(CAS, "style rules with selector lists; specificity (a,b,c) with :not() taking its argument's specificity"),
    ok(CAS, "@media with nested rules; media conditions decide which rules apply"),
    ok(CAS, "origins: user-agent and author; normal and !important (important reverses the origin order)"),
    ok(CAS, "inline style declarations (author origin, above every selector)"),
    ok(CAS, "source order across sheets, rules and declarations; the last winner per property name"),
    no(CAS, "@import", code::AT_IMPORT),
    no(CAS, "@font-face", code::AT_FONT_FACE),
    no(CAS, "@keyframes and its vendor forms", code::AT_KEYFRAMES),
    no(CAS, "@supports", code::AT_SUPPORTS),
    no(CAS, "@container", code::AT_CONTAINER),
    no(CAS, "@layer", code::AT_LAYER),
    no(CAS, "@namespace", code::AT_NAMESPACE),
    no(CAS, "@page", code::AT_PAGE),
    no(CAS, "@charset", code::AT_CHARSET),
    no(CAS, "every other at-rule", code::AT_OTHER),
    no(CAS, "nested rules and at-rules inside a style rule", code::NESTED_RULE),
    ok(VAR, "custom properties --name (case-sensitive), any tokens, inherited down the ancestor chain"),
    ok(VAR, "var(--name) and var(--name, fallback) with nested var() in fallbacks"),
    ok(VAR, "cycles and undefined references without fallback are guaranteed-invalid"),
    ok(VAR, "initial inherit unset revert on custom properties"),
    no(VAR, "env()", code::UNSUPPORTED_FUNCTION),
    ok(MQ, "media types all, screen (match) and print (never matches)"),
    no(MQ, "other media types (tv, speech, ...)", code::MEDIA_TYPE),
    ok(MQ, "features width height min-/max- (px, em, rem) and range syntax (width >= 600px)"),
    ok(MQ, "features orientation prefers-color-scheme prefers-reduced-motion hover any-hover pointer any-pointer"),
    ok(MQ, "feature resolution min-/max-resolution (dppx, x, dpi, dpcm)"),
    ok(MQ, "and, or, not and comma lists; unknown features are unknown (not matching)"),
    no(MQ, "every other media feature (color, aspect-ratio, scripting, grid, ...)", code::MEDIA_FEATURE),
];

/// Diagnostic codes with their meaning, for printing.
pub const CODES: &[(&str, &str)] = &[
    (code::INVALID_DECLARATION, "invalid declaration"),
    (code::INVALID_SELECTOR, "invalid selector syntax"),
    (code::INVALID_RULE, "rule or at-rule missing its block"),
    (code::UNEXPECTED_CLOSE_BRACE, "unexpected } in a rule prelude"),
    (code::UNCLOSED_BLOCK, "block not closed before end of input"),
    (code::INVALID_VALUE_TOKEN, "bad token in a declaration value"),
    (code::INVALID_MEDIA_QUERY, "invalid media query"),
    (code::HTML_COMMENT_TOKEN, "HTML comment token ignored"),
    (code::INVALID_ATTRIBUTE_FLAG, "attribute selector flag other than i"),
    (code::AT_IMPORT, "@import unsupported"),
    (code::AT_FONT_FACE, "@font-face unsupported"),
    (code::AT_KEYFRAMES, "@keyframes unsupported"),
    (code::AT_SUPPORTS, "@supports unsupported"),
    (code::AT_CONTAINER, "@container unsupported"),
    (code::AT_LAYER, "@layer unsupported"),
    (code::AT_NAMESPACE, "@namespace unsupported"),
    (code::AT_PAGE, "@page unsupported"),
    (code::AT_CHARSET, "@charset unsupported"),
    (code::AT_OTHER, "other at-rule unsupported"),
    (code::UNSUPPORTED_FUNCTION, "env() unsupported"),
    (code::MEDIA_TYPE, "media type unsupported"),
    (code::MEDIA_FEATURE, "media feature unsupported"),
    (code::SEL_HAS, ":has() unsupported"),
    (code::SEL_IS_WHERE, ":is() / :where() unsupported"),
    (code::SEL_LINK_HISTORY, "link and history pseudo-classes unsupported"),
    (code::SEL_PSEUDO_ELEMENT, "pseudo-elements unsupported"),
    (code::SEL_LANG_DIR, ":lang() / :dir() unsupported"),
    (code::SEL_OF_TYPE, "of-type pseudo-classes unsupported"),
    (code::SEL_NAMESPACE, "namespace prefixes unsupported"),
    (code::SEL_COMPLEX_NOT, "complex selector in :not() unsupported"),
    (code::SEL_COLUMN, "column combinator unsupported"),
    (code::SEL_NTH_OF, ":nth-child(of S) unsupported"),
    (code::SEL_PSEUDO_CLASS, "other pseudo-class unsupported"),
    (code::NESTED_RULE, "nested rule in a style rule unsupported"),
    (code::NESTING_SELECTOR, "nesting selector & unsupported"),
    (code::LIMIT_SOURCE, "resource limit: source size"),
    (code::LIMIT_NESTING, "resource limit: nesting depth"),
    (code::LIMIT_RULES, "resource limit: rule count"),
    (code::LIMIT_SELECTOR, "resource limit: selector ancestor searches"),
];

/// The allowlist as printable text: one line per entry, then the diagnostic codes.
pub fn render() -> String {
    let mut out = String::new();
    let mut row = "";
    for e in SUPPORT {
        if e.design != row {
            row = e.design;
            out.push_str(&format!("[{row}]\n"));
        }
        match e.status {
            Status::Supported => out.push_str(&format!("  supported    {}\n", e.feature)),
            Status::Unsupported(c) => out.push_str(&format!("  rejected {c} {}\n", e.feature)),
        }
    }
    out.push_str("[diagnostic codes]\n");
    for (c, what) in CODES {
        out.push_str(&format!("  {c} {what}\n"));
    }
    out
}
