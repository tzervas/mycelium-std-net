//! Op × guarantee matrix for std.net (DN-66 / VR-5 honesty).

/// One row of the guarantee matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuaranteeRow {
    /// Op name (Rust / wild bare name).
    pub op: &'static str,
    /// Guarantee tag: always `Declared` for ambient network I/O in v0.
    pub guarantee: &'static str,
    /// Effect class (blocking ambient I/O).
    pub effect: &'static str,
}

/// Frozen v0 matrix. Ambient network is **Declared** only.
pub const GUARANTEE_MATRIX: &[GuaranteeRow] = &[GuaranteeRow {
    op: "http_request",
    guarantee: "Declared",
    effect: "ambient network I/O (blocking-hypha)",
}];
