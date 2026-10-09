//! Style diagnostics. A declaration that the cascade ignores or refuses produces one, with a stable code, the property
//! name, a message and a hint. Codes are VNCSS51xx; vn-css owns VNCSS1xxx-5004 (parse, selector, media and limits).

use vn_dom::NodeId;

pub mod code {
    /// VNCSS5101: the property is not in the documented allowlist; the declaration has no effect.
    pub const UNSUPPORTED_PROPERTY: &str = "VNCSS5101";
    /// VNCSS5102: a value that browsers accept but the documented subset does not (unit, keyword, feature).
    pub const UNSUPPORTED_VALUE: &str = "VNCSS5102";
    /// VNCSS5103: a function outside the subset (min(), max(), clamp(), color-mix(), perspective(), ...).
    pub const UNSUPPORTED_FUNCTION: &str = "VNCSS5103";
    /// VNCSS5104: not a valid value for the property; ignored, as browsers ignore it.
    pub const INVALID_VALUE: &str = "VNCSS5104";
    /// VNCSS5105: `revert` is treated as `unset` (the user-agent rollback is not modelled).
    pub const REVERT_AS_UNSET: &str = "VNCSS5105";
    /// VNCSS5106: parsed and represented, but no target renders it yet (box-shadow, grid-template-areas).
    pub const NOT_RENDERED: &str = "VNCSS5106";
}

/// One diagnostic. `sheet` is the index of the author stylesheet, or None for an inline `style` attribute; `span`
/// is the byte range of the declaration in that source text.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleDiagnostic {
    pub code: &'static str,
    pub property: String,
    pub message: String,
    pub hint: &'static str,
    pub node: Option<NodeId>,
    pub sheet: Option<usize>,
    pub span: Option<(usize, usize)>,
}
